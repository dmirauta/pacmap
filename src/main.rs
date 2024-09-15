use std::{cell::RefCell, collections::HashMap, f32::consts::PI};

use egui::{Pos2, ScrollArea, Vec2};
use egui_graphs::{GraphView, Node, SettingsInteraction, SettingsNavigation, SettingsStyle};
use egui_inspect::{eframe, egui, EframeMain, EguiInspect};

use clap::Parser;
use fgraph::EguiForceGraph;
use package_info::{pacman_queery, PackageInfo, PackageName};
use petgraph::graph::NodeIndex;

mod fgraph;
mod package_info;

thread_local! {
    static NEXT: RefCell<Option<String>> = Default::default();
}

#[derive(Parser)]
struct PacmapArgs {
    #[arg(short, long)]
    /// Package first highlighted
    starting_package: Option<String>,
}

#[derive(EframeMain)]
#[eframe_main(no_eframe_app_derive)]
struct Pacmap {
    current: String,
    graph_indices: HashMap<String, NodeIndex>,
    // TODO: Can clone requirement be avoided? Are clones made?
    package_infos: EguiForceGraph<Option<PackageInfo>, ()>,
    history: Vec<PackageName>,
}

impl Pacmap {
    fn add_package(&mut self, name: String, pio: Option<PackageInfo>, pos: Pos2) -> NodeIndex {
        match self.graph_indices.get(&name) {
            Some(gi) => {
                *self
                    .package_infos
                    .egui_graph
                    .node_mut(*gi)
                    .unwrap()
                    .payload_mut() = pio;
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
        let base_pos = match self.get_package_node(&self.current) {
            Some(n) => n.location(),
            None => Pos2::ZERO,
        };
        let ni = self.add_package(name, Some(pi.clone()), base_pos);

        let sep = 15.0;
        let n = pi.depends.len() as f32;
        let dth = PI / n;
        for (i, dep) in pi.depends.iter().enumerate() {
            let th = (i as f32) * dth;
            let di = self.add_package(
                dep.0.clone(),
                None,
                base_pos + sep * Vec2::new(th.cos(), th.sin()),
            );
            self.package_infos
                .add_edge_with_label(ni, di, (), "".into());
        }
    }

    fn get_package_node(&self, name: &String) -> Option<&Node<Option<PackageInfo>, ()>> {
        let gi = self.graph_indices.get(name)?;
        self.package_infos.egui_graph.g.node_weight(*gi)
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
        let mut gv = GraphView::<_, _>::new(&mut self.package_infos.egui_graph)
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
            package_infos: EguiForceGraph::empty(),
            current: current.clone(),
        };

        if let Some((name, pi)) = pacman_queery(current.as_str()) {
            new.add_package_and_deps(name, pi);
        }

        new
    }
}

impl eframe::App for Pacmap {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let current = self.current.clone();

        egui::SidePanel::left("left").show(ctx, |ui| {
            ScrollArea::vertical().id_source("left col").show(ui, |ui| {
                match self.get_package_info(&current) {
                    Some(pi) => pi.inspect(
                        format!("currently selected package ({current})").as_str(),
                        ui,
                    ),
                    None => format!(
                        "No package info for {}, try a different package (relaunch if first).",
                        &self.current
                    )
                    .inspect("", ui),
                }
            });
        });

        if let Some(next_s) = NEXT.with_borrow_mut(|n| n.take()) {
            if !self.graph_indices.contains_key(&next_s) {
                if let Some((name, pi)) = pacman_queery(next_s.as_str()) {
                    self.add_package_and_deps(name, pi);
                }
            }
            self.history.push(PackageName(next_s.clone()));
            self.current = next_s;
        }

        egui::SidePanel::right("right").show(ctx, |ui| {
            ui.label("Hover here for help").on_hover_text_at_pointer(
                "Click&drag with LMB,
CTRL+scroll wheel for zoom,
select packages to be added to the graph in the left&right pannels.",
            );
            ScrollArea::vertical()
                .id_source("right col")
                .show(ui, |ui| {
                    self.package_infos
                        .sim_settings
                        .inspect_mut("force graph settings", ui);
                    self.history.inspect("selection history", ui);
                });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            self.inspect_graph(ui);
        });

        self.package_infos.update();
    }
}
