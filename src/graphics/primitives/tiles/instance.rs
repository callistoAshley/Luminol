// Copyright (C) 2023 Lily Lyons
//
// This file is part of Luminol.
//
// Luminol is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Luminol is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Luminol.  If not, see <http://www.gnu.org/licenses/>.
use crate::prelude::*;
use primitives::Quad;

use wgpu::util::DeviceExt;

#[derive(Debug)]
pub struct Instances {
    instance_buffer: wgpu::Buffer,
    vertex_buffer: wgpu::Buffer,

    map_width: usize,
    map_height: usize,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
struct Instance {
    position: [f32; 3],
    tile_id: i32, // force this to be an i32 to avoid padding issues
}

const TILE_QUAD: Quad = Quad::new(
    egui::Rect::from_min_max(egui::pos2(0., 0.), egui::pos2(32., 32.0)),
    // slightly smaller than 32x32 to reduce bleeding from adjacent pixels in the atlas
    egui::Rect::from_min_max(egui::pos2(0.01, 0.01), egui::pos2(31.99, 31.99)),
    0.0,
);

impl Instances {
    pub fn new(map_data: &Table3, atlas_size: wgpu::Extent3d) -> Self {
        let instances = Self::calculate_instances(map_data);
        let instance_buffer =
            state!()
                .render_state
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("tilemap tiles instance buffer"),
                    contents: bytemuck::cast_slice(&instances),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                });

        let (vertex_buffer, _) = Quad::into_buffer(&[TILE_QUAD], atlas_size);

        Self {
            instance_buffer,
            vertex_buffer,

            map_width: map_data.xsize(),
            map_height: map_data.ysize(),
        }
    }

    // I thought we didn't need the z? Well.. we do! To calculate the offset into the instance buffer.
    pub fn set_tile(&self, tile_id: i16, position: (usize, usize, usize)) {
        let offset = position.0
            + (position.1 * self.map_width)
            + (position.2 * self.map_width * self.map_height);
        let offset = offset * std::mem::size_of::<Instance>();
        state!().render_state.queue.write_buffer(
            &self.instance_buffer,
            offset as wgpu::BufferAddress,
            bytemuck::bytes_of(&Instance {
                position: [position.0 as f32, position.1 as f32, 0.0],
                tile_id: tile_id as i32,
            }),
        )
    }

    fn calculate_instances(map_data: &Table3) -> Vec<Instance> {
        map_data
            .iter()
            .copied()
            .enumerate()
            // Previously we'd filter out tiles that would not display (anything < 48).
            // However, storing the entire map like this makes it easier to edit tiles without remaking the entire buffer.
            // It's a memory tradeoff for a lot of performance.
            .map(|(index, tile_id)| {
                // We reset the x every xsize elements.
                let map_x = index % map_data.xsize();
                // We reset the y every ysize elements, but only increment it every xsize elements.
                let map_y = (index / map_data.xsize()) % map_data.ysize();
                // We don't need the z.

                Instance {
                    position: [
                        map_x as f32,
                        map_y as f32,
                        0., // We don't do a depth buffer. z doesn't matter
                    ],
                    tile_id: tile_id as i32,
                }
            })
            .collect_vec()
    }

    pub fn draw<'rpass>(&'rpass self, render_pass: &mut wgpu::RenderPass<'rpass>, layer: usize) {
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));

        // Calculate the start and end index of the buffer, as well as the amount of instances.
        let start_index = layer * self.map_width * self.map_height;
        let end_index = (layer + 1) * self.map_width * self.map_height;
        let count = (end_index - start_index) as u32;

        // Convert the indexes into actual offsets.
        let start = (start_index * std::mem::size_of::<Instance>()) as wgpu::BufferAddress;
        let end = (end_index * std::mem::size_of::<Instance>()) as wgpu::BufferAddress;

        render_pass.set_vertex_buffer(1, self.instance_buffer.slice(start..end));

        render_pass.draw(0..6, 0..count);
    }

    pub const fn desc() -> wgpu::VertexBufferLayout<'static> {
        const ARRAY: &[wgpu::VertexAttribute] =
            &wgpu::vertex_attr_array![2 => Float32x3, 3 => Sint32];
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Instance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: ARRAY,
        }
    }
}
