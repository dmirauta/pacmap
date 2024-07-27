use egui::Pos2;
use egui_inspect::{EguiInspect, InspectNumber};
use fdg::{
    fruchterman_reingold::{FruchtermanReingold, FruchtermanReingoldConfiguration},
    nalgebra::{Const, OPoint},
    Force, ForceGraph,
};
use petgraph::{graph::NodeIndex, stable_graph::StableGraph};

#[derive(EguiInspect, Clone)]
pub struct SettingsSimulation {
    #[inspect(slider, min = 0.001, max = 1.0)]
    pub dt: f32,
    #[inspect(slider, min = 0.001, max = 1.0)]
    pub cooloff_factor: f32,
    #[inspect(slider, min = 1.0, max = 1000.0)]
    pub scale: f32,
    pub active: bool,
}

impl Default for SettingsSimulation {
    fn default() -> Self {
        let FruchtermanReingoldConfiguration {
            dt,
            cooloff_factor,
            scale,
        } = Default::default();
        Self {
            dt,
            cooloff_factor,
            scale,
            active: false,
        }
    }
}

impl SettingsSimulation {
    fn make_force(&self) -> FruchtermanReingold<f32, 2> {
        FruchtermanReingold {
            conf: FruchtermanReingoldConfiguration {
                dt: self.dt,
                cooloff_factor: self.cooloff_factor,
                scale: self.scale,
            },
            ..Default::default()
        }
    }
}

fn find_matching_node(
    sg: &StableGraph<NodeIndex, (NodeIndex, NodeIndex)>,
    sni: NodeIndex,
) -> NodeIndex {
    sg.node_indices().find(|ni| *ni == sni).unwrap()
}

fn make_idx_graph<N: Clone, E: Clone>(
    g: &egui_graphs::Graph<N, E>,
) -> StableGraph<NodeIndex, (NodeIndex, NodeIndex)> {
    let mut sg = StableGraph::new();
    g.g.node_indices().for_each(|ni| {
        sg.add_node(ni);
    });
    g.g.edge_indices().for_each(|ei| {
        if let Some((ns, ne)) = g.g.edge_endpoints(ei) {
            let corresponding_start = find_matching_node(&sg, ns);
            let corresponding_end = find_matching_node(&sg, ne);
            sg.add_edge(corresponding_start, corresponding_end, (ns, ne));
        }
    });
    sg
}

// A wrapper around two wrappers around stable graph... theres probably a much nicer way to do this
pub struct EguiForceGraph<N: Clone, E: Clone> {
    pub egui_graph: egui_graphs::Graph<N, E>,
    // not sure if strictly necessary, but just in case indices don't match
    pub sim_graph: ForceGraph<f32, 2, NodeIndex, (NodeIndex, NodeIndex)>,
    pub force: FruchtermanReingold<f32, 2>,
    pub sim_settings: SettingsSimulation,
}

impl<N: Clone, E: Clone> EguiForceGraph<N, E> {
    pub fn new(
        mut egui_graph: egui_graphs::Graph<N, E>,
        sim_settings: SettingsSimulation,
        initial_perturbation: bool,
    ) -> Self {
        let mut force = sim_settings.make_force();
        let mut sim_graph = fdg::init_force_graph_uniform(make_idx_graph(&egui_graph), 1.0);

        if initial_perturbation {
            force.apply(&mut sim_graph);
            egui_graph.g.node_weights_mut().for_each(|node| {
                let point = sim_graph.node_weight(node.id()).unwrap().1;
                node.set_location(Pos2::new(point.coords.x, point.coords.y));
            });
        }

        Self {
            egui_graph,
            sim_graph,
            force,
            sim_settings,
        }
    }

    pub fn empty() -> Self {
        Self::new(
            egui_graphs::Graph::from(&StableGraph::new()),
            Default::default(),
            false,
        )
    }

    pub fn add_node_with_label_and_location(
        &mut self,
        node_data: N,
        label: String,
        pos: Pos2,
    ) -> NodeIndex {
        let g_idx =
            self.egui_graph
                .add_node_with_label_and_location(node_data.clone(), label, pos.clone());
        self.sim_graph
            .add_node((g_idx, OPoint::<_, Const<2>>::new(pos.x, pos.y)))
    }

    pub fn add_edge_with_label(
        &mut self,
        start: NodeIndex,
        end: NodeIndex,
        edge_data: E,
        label: String,
    ) {
        let g_idx =
            self.egui_graph
                .add_edge_with_label(start, end, edge_data.clone(), label.clone());
        let endpoints = self.egui_graph.edge_endpoints(g_idx).unwrap();
        self.sim_graph.add_edge(start, end, endpoints);
    }

    pub fn sync_graph_pos_to_sim(&mut self) {
        self.egui_graph.g.node_weights_mut().for_each(|node| {
            let sim_computed_point: OPoint<f32, Const<2>> =
                self.sim_graph.node_weight(node.id()).unwrap().1;
            node.set_location(Pos2::new(
                sim_computed_point.coords.x,
                sim_computed_point.coords.y,
            ));
        });
    }

    // node drag feedback
    pub fn sync_sim_pos_to_graph(&mut self) {
        self.sim_graph
            .node_weights_mut()
            .for_each(|(node_idx, loc)| {
                if let Some(g_node) = self.egui_graph.node(*node_idx) {
                    let g_point = g_node.location();
                    loc.x = g_point.x;
                    loc.y = g_point.y;
                }
            });
    }

    #[inline]
    pub fn update_forces(&mut self) {
        if self.sim_settings.active {
            self.force = self.sim_settings.make_force();
        }
    }

    #[inline]
    pub fn update_simulation(&mut self) {
        if self.sim_settings.active {
            self.force.apply(&mut self.sim_graph);
        }
    }

    pub fn update(&mut self) {
        self.sync_sim_pos_to_graph(); // apply user interaction
        self.update_forces(); // ideally running just on change
        self.update_simulation();
        self.sync_graph_pos_to_sim();
    }
}
