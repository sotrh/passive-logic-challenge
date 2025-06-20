pub mod visualization;

use core::f32;
use std::{
    collections::HashMap,
    ops::{Add, AddAssign},
};

#[derive(Debug)]
pub struct Environment {
    sun_angle: f32,
    sun_irradiance: f32,
    cloud_cover: f32,
    ambient_temp: f32,
}

impl Default for Environment {
    fn default() -> Self {
        Self {
            sun_angle: f32::consts::FRAC_PI_2,
            sun_irradiance: 1000.0,
            cloud_cover: Default::default(),
            ambient_temp: 20.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Simulation {
    nodes: Vec<Node>,
    connections: Vec<Connection>,
    solar_panels: HashMap<usize, SolarPanel>,
}

impl Simulation {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            connections: Vec::new(),
            solar_panels: HashMap::new(),
        }
    }

    pub fn add_node(
        &mut self,
        volume: f32,
        temp: f32,
        insulation: f32,
        capacity: f32,
        position: glam::Vec3,
    ) -> usize {
        let i = self.nodes.len();
        self.nodes.push(Node {
            fluid: Fluid { volume, temp },
            insulation,
            capacity,
            position,
        });
        i
    }

    pub fn get_node(&self, node: usize) -> Option<&'_ Node> {
        self.nodes.get(node)
    }

    pub fn connect_node(&mut self, input: usize, output: usize, flow_rate: f32) {
        if input < self.nodes.len() && output < self.nodes.len() {
            self.connections.push(Connection {
                flow_rate,
                input,
                output,
            });
        }
    }

    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    pub fn connections(&self) -> &[Connection] {
        &self.connections
    }

    pub fn connected_nodes(&self) -> IterConnections {
        IterConnections {
            simulation: self,
            index: 0,
        }
    }

    pub fn attach_solar_panel(&mut self, id: usize, panel: SolarPanel) {
        if self.contains_node(id) {
            self.solar_panels.insert(id, panel);
        }
    }

    pub fn tick(&mut self, environment: &Environment, dt: f32) {
        self.handle_heat_losses(environment, dt);
        self.handle_solar_panels(environment, dt);
        self.handle_fluid_transfer(dt);
    }

    fn handle_heat_losses(&mut self, environment: &Environment, dt: f32) {
        for node in &mut self.nodes {
            let temp_diff = node.fluid.temp - environment.ambient_temp;
            node.fluid.temp -= temp_diff * (1.0 - node.insulation) * dt;
        }
    }

    fn handle_solar_panels(&mut self, environment: &Environment, dt: f32) {
        for (node, panel) in &self.solar_panels {
            let node = &mut self.nodes[*node];

            if node.fluid.volume == 0.0 {
                continue;
            }

            let q = environment.sun_irradiance
                * dbg!(environment.sun_angle.sin().max(0.0))
                * (1.0 - environment.cloud_cover)
                * panel.area
                * dt
                * panel.efficiency;

            // assuming fluid is water and volume is in mL
            let density = 1.0; // g / mL
            let c = 4.186; // J / (g deg C)
            let m = node.fluid.volume * density; // g
            let d_temp = q / (m * c);

            // log::debug!("d_temp: {d_temp} C, fluid: {:?}", node.fluid);
            // log::debug!("{environment:?}");

            node.fluid.temp += d_temp;
        }
    }

    fn handle_fluid_transfer(&mut self, dt: f32) {
        for connection in &self.connections {
            if !self.contains_node(connection.input)
                || !self.contains_node(connection.output)
                || connection.input == connection.output
            {
                continue;
            }

            let amount_available = connection
                .flow_rate
                .min(self.nodes[connection.input].fluid.volume);
            let space_available =
                self.nodes[connection.output].capacity - self.nodes[connection.output].fluid.volume;

            let amount_transfered = (amount_available * dt).min(space_available);

            self.nodes[connection.input].fluid.volume -= amount_transfered;

            let fluid_transferred = Fluid {
                temp: self.nodes[connection.input].fluid.temp,
                volume: amount_transfered,
            };

            self.nodes[connection.output].fluid += fluid_transferred;
        }
    }

    pub fn contains_node(&self, id: usize) -> bool {
        id < self.nodes.len()
    }
}

pub struct IterConnections<'a> {
    simulation: &'a Simulation,
    index: usize,
}

impl<'a> Iterator for IterConnections<'a> {
    type Item = (f32, &'a Node, &'a Node);

    fn next(&mut self) -> Option<Self::Item> {
        let mut out = None;

        while self.index < self.simulation.connections.len() {
            let connection = &self.simulation.connections[self.index];

            if self.simulation.nodes.len() > connection.input
                && self.simulation.nodes.len() > connection.output
            {
                out = Some((
                    connection.flow_rate,
                    &self.simulation.nodes[connection.input],
                    &self.simulation.nodes[connection.output],
                ))
            }

            self.index += 1;
        }

        out
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fluid {
    pub volume: f32,
    pub temp: f32,
}

impl Add for Fluid {
    type Output = Fluid;

    fn add(self, rhs: Self) -> Self::Output {
        let volume = self.volume + rhs.volume;
        let temp = if volume == 0.0 {
            0.0
        } else {
            self.temp * self.volume / volume + rhs.temp * rhs.volume / volume
        };

        Self { volume, temp }
    }
}

impl AddAssign for Fluid {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

#[derive(Debug, Clone)]
pub struct Node {
    pub fluid: Fluid,
    pub capacity: f32,
    pub insulation: f32,
    pub position: glam::Vec3,
}

#[derive(Debug, Clone)]
pub struct Connection {
    pub flow_rate: f32,
    pub input: usize,
    pub output: usize,
}

#[derive(Debug, Clone)]
pub struct SolarPanel {
    pub area: f32,
    pub efficiency: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fluid_add() {
        let a = Fluid {
            volume: 1.0,
            temp: 30.0,
        };
        let b = Fluid {
            volume: 2.0,
            temp: 90.0,
        };

        assert_eq!(
            a + b,
            Fluid {
                volume: 3.0,
                temp: 70.0
            }
        );
    }

    #[test]
    fn test_heat_loss() {
        let environment = Environment::default();
        let mut simulation = Simulation::new();

        let num_nodes = 10;
        let nodes = (0..num_nodes)
            .map(|i| {
                let x = i as f32;
                simulation.add_node(x, x * 10.0, 0.9, 100.0, glam::Vec3 { x, y: 0.0, z: 0.0 })
            })
            .collect::<Vec<_>>();

        let original_sim = simulation.clone();

        simulation.handle_heat_losses(&environment, 0.1);

        for node in nodes {
            let original = original_sim.get_node(node).unwrap();
            let updated = simulation.get_node(node).unwrap();

            assert_eq!(original.fluid.volume, updated.fluid.volume);

            if original.fluid.temp < environment.ambient_temp {
                assert!(original.fluid.temp < updated.fluid.temp);
            } else if original.fluid.temp > environment.ambient_temp {
                assert!(original.fluid.temp > updated.fluid.temp);
            } else {
                assert_eq!(original.fluid.temp, updated.fluid.temp);
            }
        }
    }

    #[test]
    fn test_fluid_transfer() {
        let mut sim = Simulation::new();

        let a = sim.add_node(10.0, 100.0, 1.0, 100.0, glam::Vec3::ZERO);
        let b = sim.add_node(10.0, 20.0, 1.0, 100.0, glam::Vec3::ZERO);

        sim.connect_node(a, b, 1.0);

        let original = sim.clone();

        sim.handle_fluid_transfer(1.0);

        assert!(
            original.get_node(a).unwrap().fluid.volume > sim.get_node(a).unwrap().fluid.volume,
            "{}",
            sim.get_node(a).unwrap().fluid.volume
        );
        assert!(
            original.get_node(b).unwrap().fluid.volume < sim.get_node(b).unwrap().fluid.volume,
            "{}",
            sim.get_node(a).unwrap().fluid.volume
        );
    }
}
