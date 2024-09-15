use std::{cell::RefCell, collections::HashMap, f32::consts::PI};

use egui::{Pos2, ScrollArea, Vec2};
use egui_graphs::{GraphView, Node, SettingsInteraction, SettingsNavigation, SettingsStyle};
use egui_inspect::{eframe, egui, EframeMain, EguiInspect};

use clap::Parser;
use fgraph::EguiForceGraph;
use package_info::{pacman_queery, pacman_queery_all, PackageInfo, PackageName};
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
    /// queery all package infos at the start rather than one by one
    #[arg(short, long)]
    preload_infos: bool,
}

#[derive(EframeMain)]
#[eframe_main(no_eframe_app_derive)]
struct Pacmap {
    current: String,
    package_infos: HashMap<String, PackageInfo>,
    // TODO: it is redundant to store a String, just use node label?
    graph: EguiForceGraph<(), ()>,
    /// indices by label
    graph_indices: HashMap<String, NodeIndex>,
    history: Vec<PackageName>,
}

impl Pacmap {
    fn add_package(&mut self, name: String, pos: Pos2) -> NodeIndex {
        match self.graph_indices.get(&name) {
            Some(gi) => *gi,
            None => {
                let gi = self
                    .graph
                    .add_node_with_label_and_location((), name.clone(), pos);
                self.graph_indices.insert(name, gi);
                gi
            }
        }
    }

    fn get_package_node(&self, name: &String) -> Option<&Node<(), ()>> {
        let gi = self.graph_indices.get(name)?;
        self.graph.egui_graph.g.node_weight(*gi)
    }

    fn add_package_and_deps(&mut self, name: String) {
        if !self.package_infos.contains_key(&name) {
            if let Some((name, pi)) = pacman_queery(&name) {
                self.package_infos.insert(name, pi);
            }
        }
        let depends = match self.package_infos.get(&name) {
            Some(pi) => pi.depends.clone(),
            None => vec![],
        };

        let base_pos = match self.get_package_node(&self.current) {
            Some(n) => n.location(),
            None => Pos2::ZERO,
        };
        let ni = self.add_package(name.clone(), base_pos);

        let sep = 15.0;
        let n = depends.len() as f32;
        let dth = PI / n;
        for (i, dep) in depends.iter().enumerate() {
            let th = (i as f32) * dth;
            let di = self.add_package(
                dep.0.clone(),
                base_pos + sep * Vec2::new(th.cos(), th.sin()),
            );
            self.graph.add_unique_edge_with_label(ni, di, (), "".into());
        }
    }

    fn inspect_graph(&mut self, ui: &mut egui::Ui) {
        let interaction_settings = SettingsInteraction::new().with_dragging_enabled(true);
        let settings_navigation = SettingsNavigation::new()
            .with_zoom_and_pan_enabled(true)
            .with_fit_to_screen_enabled(false);
        let style_settings = SettingsStyle::new().with_labels_always(true);
        let mut gv = GraphView::<_, _>::new(&mut self.graph.egui_graph)
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

        let package_infos = match args.preload_infos {
            true => pacman_queery_all(),
            false => HashMap::new(),
        };

        let mut new = Self {
            history: vec![PackageName(current.clone())],
            graph_indices: HashMap::new(),
            graph: EguiForceGraph::empty(),
            package_infos,
            current: current.clone(),
        };

        new.add_package_and_deps(current);

        new
    }
}

impl eframe::App for Pacmap {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let current = self.current.clone();

        egui::SidePanel::left("left").show(ctx, |ui| {
            ScrollArea::vertical().id_source("left col").show(ui, |ui| {
                match self.package_infos.get(&current) {
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
            self.add_package_and_deps(next_s.clone());
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
                    self.graph
                        .sim_settings
                        .inspect_mut("force graph settings", ui);
                    self.history.inspect("selection history", ui);
                });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            self.inspect_graph(ui);
        });

        self.graph.update();
    }
}
