pub mod visualization;

use core::f32;
use std::ops::{Add, AddAssign};

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
}

impl Simulation {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            connections: Vec::new(),
        }
    }

    pub fn add_node(
        &mut self,
        volume: f32,
        temp: f32,
        insulation: f32,
        position: glam::Vec3,
    ) -> usize {
        let i = self.nodes.len();
        self.nodes.push(Node {
            fluid: Fluid { volume, temp },
            insulation,
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

    pub fn tick(&mut self, environment: &Environment, dt: f32) {
        self.handle_heat_losses(environment, dt);
    }

    fn handle_heat_losses(&mut self, environment: &Environment, dt: f32) {
        for node in &mut self.nodes {
            let temp_diff = node.fluid.temp - environment.ambient_temp;
            node.fluid.temp -= temp_diff * (1.0 - node.insulation) * dt;
        }
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
    volume: f32,
    temp: f32,
}

impl Add for Fluid {
    type Output = Fluid;

    fn add(self, rhs: Self) -> Self::Output {
        let volume = self.volume + rhs.volume;
        Self {
            volume,
            temp: self.temp * self.volume / volume + rhs.temp * rhs.volume / volume,
        }
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
    pub insulation: f32,
    pub position: glam::Vec3,
}

#[derive(Debug, Clone)]
pub struct Connection {
    pub flow_rate: f32,
    pub input: usize,
    pub output: usize,
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
                simulation.add_node(x, x * 10.0, 0.9, glam::Vec3 { x, y: 0.0, z: 0.0 })
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
}
