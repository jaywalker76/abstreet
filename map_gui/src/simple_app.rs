use abstutil::{CmdArgs, Timer};
use geom::{Circle, Distance, Duration, Pt2D, Time};
use map_model::{IntersectionID, Map};
use sim::Sim;
use widgetry::{Canvas, EventCtx, GfxCtx, SharedAppState, State, Transition, Warper};

use crate::colors::ColorScheme;
use crate::options::Options;
use crate::render::DrawMap;
use crate::render::{DrawOptions, Renderable};
use crate::{AppLike, ID};

/// Simple app state that just renders a static map, without any dynamic agents on the map.
pub struct SimpleApp<T> {
    pub map: Map,
    pub draw_map: DrawMap,
    pub cs: ColorScheme,
    pub opts: Options,
    pub current_selection: Option<ID>,
    /// Custom per-app state can be stored here
    pub session: T,
}

impl<T> SimpleApp<T> {
    pub fn new(ctx: &mut EventCtx, args: CmdArgs, session: T) -> SimpleApp<T> {
        SimpleApp::new_with_opts(ctx, args, Options::default(), session)
    }

    pub fn new_with_opts(
        ctx: &mut EventCtx,
        mut args: CmdArgs,
        mut opts: Options,
        session: T,
    ) -> SimpleApp<T> {
        ctx.loading_screen("load map", |ctx, mut timer| {
            opts.update_from_args(&mut args);
            let map_path = args
                .optional_free()
                .unwrap_or(abstutil::MapName::seattle("montlake").path());
            args.done();

            let cs = ColorScheme::new(ctx, opts.color_scheme);
            let map = Map::new(map_path, &mut timer);
            let draw_map = DrawMap::new(ctx, &map, &opts, &cs, timer);
            ctx.canvas.load_camera_state(map.get_name());
            SimpleApp {
                map,
                draw_map,
                cs,
                opts,
                current_selection: None,
                session,
            }
        })
    }

    pub fn draw_unzoomed(&self, g: &mut GfxCtx) {
        g.clear(self.cs.void_background);
        g.redraw(&self.draw_map.boundary_polygon);
        g.redraw(&self.draw_map.draw_all_areas);
        g.redraw(&self.draw_map.draw_all_unzoomed_parking_lots);
        g.redraw(&self.draw_map.draw_all_unzoomed_roads_and_intersections);
        g.redraw(&self.draw_map.draw_all_buildings);
        // Not the building paths

        // Still show some shape selection when zoomed out.
        // TODO Refactor! Ideally use get_obj
        if let Some(ID::Area(id)) = self.current_selection {
            g.draw_polygon(
                self.cs.selected,
                self.draw_map.get_a(id).get_outline(&self.map),
            );
        } else if let Some(ID::Road(id)) = self.current_selection {
            g.draw_polygon(
                self.cs.selected,
                self.draw_map.get_r(id).get_outline(&self.map),
            );
        } else if let Some(ID::Intersection(id)) = self.current_selection {
            // Actually, don't use get_outline here! Full polygon is easier to see.
            g.draw_polygon(self.cs.selected, self.map.get_i(id).polygon.clone());
        } else if let Some(ID::Building(id)) = self.current_selection {
            g.draw_polygon(self.cs.selected, self.map.get_b(id).polygon.clone());
        }
    }

    pub fn draw_zoomed(&self, g: &mut GfxCtx, opts: DrawOptions) {
        g.clear(self.cs.void_background);
        g.redraw(&self.draw_map.boundary_polygon);

        let objects = self
            .draw_map
            .get_renderables_back_to_front(g.get_screen_bounds(), &self.map);

        let mut drawn_all_buildings = false;
        let mut drawn_all_areas = false;

        for obj in objects {
            obj.draw(g, self, &opts);

            match obj.get_id() {
                ID::Building(_) => {
                    if !drawn_all_buildings {
                        if opts.show_building_paths {
                            g.redraw(&self.draw_map.draw_all_building_paths);
                        }
                        g.redraw(&self.draw_map.draw_all_buildings);
                        g.redraw(&self.draw_map.draw_all_building_outlines);
                        drawn_all_buildings = true;
                    }
                }
                ID::Area(_) => {
                    if !drawn_all_areas {
                        g.redraw(&self.draw_map.draw_all_areas);
                        drawn_all_areas = true;
                    }
                }
                _ => {}
            }

            if self.current_selection == Some(obj.get_id()) {
                g.draw_polygon(self.cs.selected, obj.get_outline(&self.map));
            }
        }
    }

    /// Assumes some defaults.
    pub fn recalculate_current_selection(&mut self, ctx: &EventCtx) {
        self.current_selection = self.calculate_current_selection(ctx, false, false);
    }

    // TODO Returns anything; I think it should just return roads
    pub fn mouseover_unzoomed_roads_and_intersections(&self, ctx: &EventCtx) -> Option<ID> {
        self.calculate_current_selection(ctx, true, false)
    }
    /// Only select buildings, and work whether zoomed in or not.
    pub fn mouseover_unzoomed_buildings(&self, ctx: &EventCtx) -> Option<ID> {
        self.calculate_current_selection(ctx, false, true)
            .filter(|id| match id {
                ID::Building(_) => true,
                _ => false,
            })
    }

    fn calculate_current_selection(
        &self,
        ctx: &EventCtx,
        unzoomed_roads_and_intersections: bool,
        unzoomed_buildings: bool,
    ) -> Option<ID> {
        // Unzoomed mode. Ignore when debugging areas.
        if ctx.canvas.cam_zoom < self.opts.min_zoom_for_detail
            && !(unzoomed_roads_and_intersections || unzoomed_buildings)
        {
            return None;
        }

        let pt = ctx.canvas.get_cursor_in_map_space()?;

        let mut objects = self.draw_map.get_renderables_back_to_front(
            Circle::new(pt, Distance::meters(3.0)).get_bounds(),
            &self.map,
        );
        objects.reverse();

        for obj in objects {
            match obj.get_id() {
                ID::Road(_) => {
                    if !unzoomed_roads_and_intersections
                        || ctx.canvas.cam_zoom >= self.opts.min_zoom_for_detail
                    {
                        continue;
                    }
                }
                ID::Intersection(_) => {
                    if ctx.canvas.cam_zoom < self.opts.min_zoom_for_detail
                        && !unzoomed_roads_and_intersections
                    {
                        continue;
                    }
                }
                ID::Building(_) => {
                    if ctx.canvas.cam_zoom < self.opts.min_zoom_for_detail && !unzoomed_buildings {
                        continue;
                    }
                }
                _ => {
                    if ctx.canvas.cam_zoom < self.opts.min_zoom_for_detail {
                        continue;
                    }
                }
            }
            if obj.contains_pt(pt, &self.map) {
                return Some(obj.get_id());
            }
        }
        None
    }
}

impl<T> AppLike for SimpleApp<T> {
    #[inline]
    fn map(&self) -> &Map {
        &self.map
    }
    #[inline]
    fn sim(&self) -> &Sim {
        unreachable!()
    }
    #[inline]
    fn cs(&self) -> &ColorScheme {
        &self.cs
    }
    #[inline]
    fn mut_cs(&mut self) -> &mut ColorScheme {
        &mut self.cs
    }
    #[inline]
    fn draw_map(&self) -> &DrawMap {
        &self.draw_map
    }
    #[inline]
    fn mut_draw_map(&mut self) -> &mut DrawMap {
        &mut self.draw_map
    }
    #[inline]
    fn opts(&self) -> &Options {
        &self.opts
    }
    #[inline]
    fn mut_opts(&mut self) -> &mut Options {
        &mut self.opts
    }

    fn map_switched(&mut self, ctx: &mut EventCtx, map: Map, timer: &mut Timer) {
        ctx.canvas.save_camera_state(self.map().get_name());
        self.map = map;
        self.draw_map = DrawMap::new(ctx, &self.map, &self.opts, &self.cs, timer);
        ctx.canvas.load_camera_state(self.map.get_name());
    }

    fn draw_with_opts(&self, g: &mut GfxCtx, opts: DrawOptions) {
        if g.canvas.cam_zoom < self.opts.min_zoom_for_detail {
            self.draw_unzoomed(g);
        } else {
            self.draw_zoomed(g, opts);
        }
    }

    fn make_warper(
        &mut self,
        ctx: &EventCtx,
        pt: Pt2D,
        target_cam_zoom: Option<f64>,
        _: Option<ID>,
    ) -> Box<dyn State<SimpleApp<T>>> {
        Box::new(SimpleWarper {
            warper: Warper::new(ctx, pt, target_cam_zoom),
        })
    }

    fn sim_time(&self) -> Time {
        Time::START_OF_DAY
    }

    fn current_stage_and_remaining_time(&self, id: IntersectionID) -> (usize, Duration) {
        (
            0,
            self.map.get_traffic_signal(id).stages[0]
                .phase_type
                .simple_duration(),
        )
    }
}

impl<T> SharedAppState for SimpleApp<T> {
    fn draw_default(&self, g: &mut GfxCtx) {
        self.draw_with_opts(g, DrawOptions::new());
    }

    fn dump_before_abort(&self, canvas: &Canvas) {
        canvas.save_camera_state(&self.map.get_name());
    }

    fn before_quit(&self, canvas: &Canvas) {
        canvas.save_camera_state(&self.map.get_name());
    }
}

struct SimpleWarper {
    warper: Warper,
}

impl<T> State<SimpleApp<T>> for SimpleWarper {
    fn event(&mut self, ctx: &mut EventCtx, _: &mut SimpleApp<T>) -> Transition<SimpleApp<T>> {
        if self.warper.event(ctx) {
            Transition::Keep
        } else {
            Transition::Pop
        }
    }

    fn draw(&self, _: &mut GfxCtx, _: &SimpleApp<T>) {}
}

/// Store a cached key/value pair, only recalculating when the key changes.
pub struct Cached<K: PartialEq + Clone, V> {
    contents: Option<(K, V)>,
}

impl<K: PartialEq + Clone, V> Cached<K, V> {
    pub fn new() -> Cached<K, V> {
        Cached { contents: None }
    }

    /// Get the current key.
    pub fn key(&self) -> Option<K> {
        self.contents.as_ref().map(|(k, _)| k.clone())
    }

    /// Get the current value.
    pub fn value(&self) -> Option<&V> {
        self.contents.as_ref().map(|(_, v)| v)
    }

    /// Update the value if the key has changed.
    pub fn update<F: FnMut(K) -> V>(&mut self, key: Option<K>, mut produce_value: F) {
        if let Some(new_key) = key {
            if self.key() != Some(new_key.clone()) {
                self.contents = Some((new_key.clone(), produce_value(new_key)));
            }
        } else {
            self.contents = None;
        }
    }

    pub fn clear(&mut self) {
        self.contents = None;
    }
}
