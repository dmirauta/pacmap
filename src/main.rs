use std::{cell::RefCell, collections::HashMap, process::Command, str::FromStr};

use egui::{Pos2, Vec2};
use egui_graphs::{Graph, GraphView, Node, SettingsInteraction, SettingsNavigation, SettingsStyle};
use egui_inspect::{EframeMain, EguiInspect};

use clap::Parser;
use petgraph::{graph::NodeIndex, stable_graph::StableGraph};

thread_local! {
    static NEXT: RefCell<Option<String>> = Default::default();
}

#[derive(Debug, Default, Clone)]
struct PackageName(String);

impl From<String> for PackageName {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl EguiInspect for PackageName {
    fn inspect(&self, _label: &str, ui: &mut egui::Ui) {
        if ui.button(self.0.as_str()).clicked() {
            NEXT.with_borrow_mut(|n| *n = Some(self.0.clone()));
        }
    }

    fn inspect_mut(&mut self, _label: &str, _ui: &mut egui::Ui) {
        todo!()
    }
}

#[derive(EguiInspect, Default, Debug, Clone)]
struct OptionalDep {
    package_name: String,
    reason: String,
}

impl FromStr for OptionalDep {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut sp = s.split(": ");
        match (sp.next(), sp.next()) {
            (Some(package_name), Some(reason)) => Ok(Self {
                package_name: package_name.trim().to_string(),
                reason: reason.into(),
            }),
            _ => Err("Unexpected OptionalDep format".into()),
        }
    }
}

// NOTE: Clone added as Graph requirement
#[derive(Debug, EguiInspect, Default, Clone)]
struct PackageInfo {
    depends: Vec<PackageName>,
    optional: Vec<OptionalDep>,
    required_by: Vec<PackageName>,
    other: HashMap<String, String>,
}

/// space separated string vec
fn sssv(s: String) -> Vec<PackageName> {
    s.split_whitespace()
        .map(|s| String::from(s).into())
        .collect()
}

fn pacman_queery(name: impl AsRef<str>) -> Option<(String, PackageInfo)> {
    let out = Command::new("pacman")
        .arg("-Qi")
        .arg(name.as_ref())
        .output();
    match out {
        Ok(o) => {
            let package_info = String::from_utf8(o.stdout).unwrap();
            if package_info.is_empty() || &package_info[..=6] == "error:" {
                None
            } else {
                Some(PackageInfo::parse(package_info))
            }
        }
        Err(e) => {
            dbg!(e);
            None
        }
    }
}

impl PackageInfo {
    fn parse(s: String) -> (String, Self) {
        let mut other = HashMap::new();
        let mut optional = vec![];

        for l in s.lines().filter(|l| !l.is_empty()) {
            let mut sp = l.split(" : ");
            let key = sp.next();
            let val = sp.next();
            if let Some(v) = val {
                let k: String = key.unwrap().into();
                if k != "Optional Deps" {
                    other.insert(k.trim().into(), v.into());
                } else {
                    optional.push(v.parse().unwrap());
                }
            } else {
                let k: String = key.unwrap().into();
                optional.push(k.parse().unwrap());
            }
        }

        let failure = format!("Failed parsing {s}");
        let name = other.remove("Name").expect(failure.as_str());
        let depends = sssv(other.remove("Depends On").expect(failure.as_str()));
        let required_by = sssv(other.remove("Required By").expect(failure.as_str()));

        (
            name,
            Self {
                depends,
                optional,
                required_by,
                other,
            },
        )
    }
}

#[derive(Parser)]
struct PacmapArgs {
    #[arg(short, long)]
    /// Package first highlighted
    starting_package: Option<String>,
}

#[derive(EframeMain)]
struct Pacmap {
    current: String,
    graph_indices: HashMap<String, NodeIndex>,
    // TODO: Can clone requirement be avoided? Are clones made?
    package_infos: Graph<Option<PackageInfo>, ()>,
    history: Vec<PackageName>,
}

impl Pacmap {
    fn add_package(&mut self, name: String, pio: Option<PackageInfo>, pos: Pos2) -> NodeIndex {
        match self.graph_indices.get(&name) {
            Some(gi) => {
                *self.package_infos.node_mut(*gi).unwrap().payload_mut() = pio;
                *gi
            }
            None => {
                let gi =
                    self.package_infos
                        .add_node_with_label_and_location(pio, name.clone(), pos);
                self.graph_indices.insert(name, gi);
                gi
            }
        }
    }

    fn add_package_and_deps(&mut self, name: String, pi: PackageInfo) {
        let mut base_pos = match self.get_package_node(&self.current) {
            Some(n) => n.location() + Vec2::new(0.0, -25.0),
            None => Pos2::ZERO,
        };
        let ni = self.add_package(name, Some(pi.clone()), base_pos);

        let sep = 15.0;
        let n = pi.depends.len() as f32;
        base_pos -= Vec2::new(sep * n * 0.5, 0.0);
        for dep in pi.depends.iter() {
            let di = self.add_package(dep.0.clone(), None, base_pos);
            self.package_infos
                .add_edge_with_label(ni, di, (), "".into());
            base_pos += Vec2::new(sep, 0.0);
        }
    }

    fn get_package_node(&self, name: &String) -> Option<&Node<Option<PackageInfo>, ()>> {
        let gi = self.graph_indices.get(name)?;
        self.package_infos.g.node_weight(*gi)
    }

    fn get_package_info(&mut self, name: &String) -> Option<&PackageInfo> {
        let node = self.get_package_node(name)?;

        if node.payload().is_none() {
            if let Some((_, pi)) = pacman_queery(name) {
                self.add_package_and_deps(name.clone(), pi);
            }
        };

        let node = self.get_package_node(name)?;
        node.payload().as_ref()
    }

    fn inspect_graph(&mut self, ui: &mut egui::Ui) {
        let interaction_settings = SettingsInteraction::new().with_dragging_enabled(true);
        let settings_navigation = SettingsNavigation::new()
            .with_zoom_and_pan_enabled(true)
            .with_fit_to_screen_enabled(false);
        let style_settings = SettingsStyle::new().with_labels_always(true);
        let mut gv = GraphView::<_, _>::new(&mut self.package_infos)
            .with_styles(&style_settings)
            .with_interactions(&interaction_settings)
            .with_navigations(&settings_navigation);
        ui.add(&mut gv);
    }
}

impl Default for Pacmap {
    fn default() -> Self {
        let args = PacmapArgs::parse();
        let current = args.starting_package.unwrap_or("pacman".into());

        let mut new = Self {
            history: vec![PackageName(current.clone())],
            graph_indices: HashMap::new(),
            package_infos: Graph::from(&StableGraph::new()),
            current: current.clone(),
        };

        if let Some((name, pi)) = pacman_queery(current.as_str()) {
            new.add_package_and_deps(name, pi);
        }

        new
    }
}

impl EguiInspect for Pacmap {
    fn inspect(&self, _label: &str, _ui: &mut egui::Ui) {
        todo!()
    }

    fn inspect_mut(&mut self, _label: &str, ui: &mut egui::Ui) {
        let current = self.current.clone();
        ui.columns(2, |columns| {
            match self.get_package_info(&current) {
                Some(pi) => pi.inspect(current.as_str(), &mut columns[0]),
                None => format!(
                    "No package info for {}, relaunch with a different starting package.",
                    &self.current
                )
                .inspect("", &mut columns[0]),
            }

            if let Some(next_s) = NEXT.with_borrow_mut(|n| n.take()) {
                if !self.graph_indices.contains_key(&next_s) {
                    if let Some((name, pi)) = pacman_queery(next_s.as_str()) {
                        self.add_package_and_deps(name, pi);
                    }
                }
                self.history.push(PackageName(next_s.clone()));
                self.current = next_s;
            }

            self.history.inspect("selection history", &mut columns[0]);
            self.inspect_graph(&mut columns[1]);
        });
    }
}
