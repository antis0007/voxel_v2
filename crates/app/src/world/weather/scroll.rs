//! Clipmap movement reuses overlapping samples without allocating snapshots.
//! CPU scrolling does not scroll the GPU texture: full uploads remain required.
use super::{CloudControls, IVec2, Vec2, WeatherClipmapLevel};
use std::sync::LazyLock;

static TRACE: LazyLock<bool> = LazyLock::new(|| std::env::var_os("VOXEL_WEATHER_TRACE").is_some());

impl WeatherClipmapLevel {
    pub(super) fn scroll_to_origin(
        &mut self,
        origin_m: Vec2,
        time_s: f32,
        texel_offset: IVec2,
        controls: CloudControls,
        weather_seed: u32,
    ) {
        let resolution = self.region.resolution as i32;
        if texel_offset.x.abs() >= resolution || texel_offset.y.abs() >= resolution {
            if *TRACE {
                eprintln!(
                    "[weather-scroll] level={} offset={texel_offset:?} full_refresh=true texels={}",
                    self.descriptor.level,
                    resolution * resolution
                );
            }
            self.full_refresh(origin_m, time_s, controls, weather_seed);
            return;
        }

        self.region.origin_m = origin_m;
        let destination_x = (-texel_offset.x).max(0) as usize;
        let source_x = texel_offset.x.max(0) as usize;
        let overlap_width = (resolution - texel_offset.x.abs()) as usize;

        for row in 0..resolution {
            // Copy towards the vacated rows. Reversing negative-Y movement
            // prevents overwriting a source row before its destination reads it.
            let y = if texel_offset.y < 0 {
                resolution - 1 - row
            } else {
                row
            };
            let source_y = y + texel_offset.y;
            let row_overlaps = (0..resolution).contains(&source_y);
            if row_overlaps {
                let source_start = self.texel_index(source_x, source_y as usize);
                let destination_start = self.texel_index(destination_x, y as usize);
                let source_range = source_start..source_start + overlap_width;
                // copy_within handles horizontal overlap within each row.
                self.primary_rgba
                    .copy_within(source_range.clone(), destination_start);
                self.secondary_rgba
                    .copy_within(source_range.clone(), destination_start);
                self.shadow_rgba
                    .copy_within(source_range, destination_start);
            }

            let exposed_columns = if row_overlaps {
                [
                    0..destination_x,
                    destination_x + overlap_width..resolution as usize,
                ]
            } else {
                [0..resolution as usize, 0..0]
            };
            for x in exposed_columns.into_iter().flatten() {
                let index = self.texel_index(x, y as usize);
                let world_xz_m = self.texel_world_position(origin_m, x, y as usize);
                let sample = self.sample_texel(world_xz_m, controls, weather_seed);
                self.write_sample(index, sample);
            }
        }

        self.mark_full_level_dirty();
        // Keep the original row-major reduction order for identical summaries.
        self.rebuild_summary();
        if *TRACE {
            let reused = overlap_width * (resolution - texel_offset.y.abs()) as usize;
            let rebuilt = resolution as usize * resolution as usize - reused;
            eprintln!(
                "[weather-scroll] level={} offset={texel_offset:?} reused={reused} rebuilt={rebuilt} full_upload=true",
                self.descriptor.level
            );
        }
    }
}
