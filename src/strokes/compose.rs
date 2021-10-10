use gtk4::{gio, glib};
use serde::{Deserialize, Serialize};
use std::ops::Deref;
use svg::node::element::path;

use crate::config;

use super::Element;

#[allow(dead_code)]
pub fn add_xml_header(svg: &str) -> String {
    let re = regex::Regex::new(r#"<\?xml[^\?>]*\?>"#).unwrap();
    if !re.is_match(svg) {
        let mut string = String::from(r#"<?xml version="1.0" standalone="no"?>"#);
        string.push('\n');
        string.push_str(svg);
        string
    } else {
        String::from(svg)
    }
}

pub fn remove_xml_header(svg: &str) -> String {
    let re = regex::Regex::new(r#"<\?xml[^\?>]*\?>"#).unwrap();
    String::from(re.replace_all(svg, ""))
}

#[allow(dead_code)]
pub fn strip_svg_root(svg: &str) -> String {
    let re = regex::Regex::new(r#"<svg[^>]*>|<[^/svg]*/svg>"#).unwrap();
    String::from(re.replace_all(svg, ""))
}

pub fn wrap_svg(
    data: &str,
    bounds: Option<p2d::bounding_volume::AABB>,
    viewbox: Option<p2d::bounding_volume::AABB>,
    xml_header: bool,
    preserve_aspectratio: bool,
) -> String {
    let mut cx = tera::Context::new();

    let (x, y, width, height) = if let Some(bounds) = bounds {
        let x = format!("{}", bounds.mins[0].floor() as i32);
        let y = format!("{}", bounds.mins[1].floor() as i32);
        let width = format!("{}", (bounds.maxs[0] - bounds.mins[0]).ceil() as i32);
        let height = format!("{}", (bounds.maxs[1] - bounds.mins[1]).ceil() as i32);

        (x, y, width, height)
    } else {
        (
            String::from("0"),
            String::from("0"),
            String::from("100%"),
            String::from("100%"),
        )
    };

    let viewbox = if let Some(viewbox) = viewbox {
        format!(
            "viewBox=\"{} {} {} {}\"",
            viewbox.mins[0].floor() as i32,
            viewbox.mins[1].floor() as i32,
            (viewbox.maxs[0] - viewbox.mins[0]).ceil() as i32,
            (viewbox.maxs[1] - viewbox.mins[1]).ceil() as i32
        )
    } else {
        String::from("")
    };
    let preserve_aspectratio = if preserve_aspectratio {
        String::from("xMidyMid")
    } else {
        String::from("none")
    };

    cx.insert("xml_header", &xml_header);
    cx.insert("data", data);
    cx.insert("x", &x);
    cx.insert("y", &y);
    cx.insert("width", &width);
    cx.insert("height", &height);
    cx.insert("viewbox", &viewbox);
    cx.insert("preserve_aspectratio", &preserve_aspectratio);

    let templ = String::from_utf8(
        gio::resources_lookup_data(
            (String::from(config::APP_IDPATH) + "templates/svg_wrap.svg.templ").as_str(),
            gio::ResourceLookupFlags::NONE,
        )
        .unwrap()
        .deref()
        .to_vec(),
    )
    .unwrap();
    let output = tera::Tera::one_off(templ.as_str(), &cx, false)
        .expect("failed to create svg from template");

    output
}

pub fn svg_intrinsic_size(svg: &str) -> Option<na::Vector2<f64>> {
    let stream = gio::MemoryInputStream::from_bytes(&glib::Bytes::from(svg.as_bytes()));
    if let Ok(handle) = librsvg::Loader::new()
        .read_stream::<gio::MemoryInputStream, gio::File, gio::Cancellable>(&stream, None, None)
    {
        let renderer = librsvg::CairoRenderer::new(&handle);

        let intrinsic_size = if let Some(size) = renderer.intrinsic_size_in_pixels() {
            Some(na::vector![size.0, size.1])
        } else {
            log::warn!("intrinsic_size_in_pixels() failed in svg_intrinsic_size()");
            None
        };

        intrinsic_size
    } else {
        None
    }
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
pub struct Line {
    pub start: na::Vector2<f64>,
    pub end: na::Vector2<f64>,
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
pub struct QuadBezier {
    pub start: na::Vector2<f64>,
    pub cp1: na::Vector2<f64>,
    pub end: na::Vector2<f64>,
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
pub struct CubicBezier {
    pub start: na::Vector2<f64>,
    pub cp1: na::Vector2<f64>,
    pub cp2: na::Vector2<f64>,
    pub end: na::Vector2<f64>,
}

pub fn vector2_unit_norm(vec: na::Vector2<f64>) -> na::Vector2<f64> {
    let rot_90deg = na::Rotation2::new(std::f64::consts::PI / 2.0);
    rot_90deg * vec.normalize()
}

pub fn linear_offsetted(
    line: Line,
    start_offset_dist: f64,
    end_offset_dist: f64,
    move_start: bool,
) -> Vec<path::Command> {
    let direction_unit_norm = vector2_unit_norm(line.end - line.start);
    let start_offset = direction_unit_norm * start_offset_dist;

    let end_offset = direction_unit_norm * end_offset_dist;

    let mut commands = Vec::new();
    if move_start {
        commands.push(path::Command::Move(
            path::Position::Absolute,
            path::Parameters::from((
                line.start[0] + start_offset[0],
                line.start[1] + start_offset[1],
            )),
        ));
    }
    commands.push(path::Command::Line(
        path::Position::Absolute,
        path::Parameters::from((line.end[0] + end_offset[0], line.end[1] + end_offset[1])),
    ));

    commands
}

pub fn linear_variable_width(line: Line, width_start: f64, width_end: f64) -> Vec<path::Command> {
    let start_offset_dist = width_start / 2.0;
    let end_offset_dist = width_end / 2.0;

    let line_reverse = Line {
        start: line.end,
        end: line.start,
    };
    let direction_unit_norm = vector2_unit_norm(line.end - line.start);

    let mut commands = Vec::new();
    commands.append(&mut linear_offsetted(
        line,
        start_offset_dist,
        end_offset_dist,
        true,
    ));
    commands.push(path::Command::EllipticalArc(
        path::Position::Absolute,
        path::Parameters::from((
            end_offset_dist,
            end_offset_dist,
            0.0,
            0.0,
            0.0,
            (line.end + direction_unit_norm * (-end_offset_dist))[0],
            (line.end + direction_unit_norm * (-end_offset_dist))[1],
        )),
    ));
    commands.append(&mut linear_offsetted(
        line_reverse,
        end_offset_dist,
        start_offset_dist,
        false,
    ));
    commands.push(path::Command::EllipticalArc(
        path::Position::Absolute,
        path::Parameters::from((
            start_offset_dist,
            start_offset_dist,
            0.0,
            0.0,
            0.0,
            (line_reverse.end + direction_unit_norm * (start_offset_dist))[0],
            (line_reverse.end + direction_unit_norm * (start_offset_dist))[1],
        )),
    ));

    commands
}

// Offsetted quad bezier approximation, see "precise offsetting of quadratic bezier curves"
pub fn quad_bezier_offsetted(
    quad_bezier: QuadBezier,
    start_offset_dist: f64,
    end_offset_dist: f64,
    move_start: bool,
) -> Vec<path::Command> {
    let start_unit_norm = vector2_unit_norm(quad_bezier.cp1 - quad_bezier.start);
    let start_offset = start_unit_norm * start_offset_dist;

    let end_unit_norm = vector2_unit_norm(quad_bezier.end - quad_bezier.cp1);
    let end_offset = end_unit_norm * end_offset_dist;

    let added_unit_norms = start_unit_norm + end_unit_norm;

    let cp1_offset_dist = (start_offset_dist + end_offset_dist) / 2.0;
    let cp1_offset =
        (2.0 * cp1_offset_dist * added_unit_norms) / added_unit_norms.dot(&added_unit_norms);

    let mut commands = Vec::new();
    if move_start {
        commands.push(path::Command::Move(
            path::Position::Absolute,
            path::Parameters::from((
                quad_bezier.start[0] + start_offset[0],
                quad_bezier.start[1] + start_offset[1],
            )),
        ));
    }
    commands.push(path::Command::QuadraticCurve(
        path::Position::Absolute,
        path::Parameters::from((
            (
                quad_bezier.cp1[0] + cp1_offset[0],
                quad_bezier.cp1[1] + cp1_offset[1],
            ),
            (
                quad_bezier.end[0] + end_offset[0],
                quad_bezier.end[1] + end_offset[1],
            ),
        )),
    ));

    commands
}

pub fn quad_bezier_variable_width(
    quad_bezier: QuadBezier,
    width_start: f64,
    width_end: f64,
) -> Vec<path::Command> {
    let mut commands = Vec::new();

    let start_offset_dist = width_start / 2.0;
    let end_offset_dist = width_end / 2.0;

    let quad_bezier_reverse = QuadBezier {
        start: quad_bezier.end,
        cp1: quad_bezier.cp1,
        end: quad_bezier.start,
    };

    commands.append(&mut quad_bezier_offsetted(
        quad_bezier,
        start_offset_dist,
        end_offset_dist,
        true,
    ));
    commands.push(path::Command::Line(
        path::Position::Absolute,
        path::Parameters::from((
            (quad_bezier.end
                + vector2_unit_norm(quad_bezier.end - quad_bezier.cp1) * (-end_offset_dist))[0],
            (quad_bezier.end
                + vector2_unit_norm(quad_bezier.end - quad_bezier.cp1) * (-end_offset_dist))[1],
        )),
    ));

    commands.append(&mut quad_bezier_offsetted(
        quad_bezier_reverse,
        end_offset_dist,
        start_offset_dist,
        false,
    ));
    commands.push(path::Command::Line(
        path::Position::Absolute,
        path::Parameters::from((
            (quad_bezier_reverse.end
                + vector2_unit_norm(quad_bezier_reverse.end - quad_bezier_reverse.cp1)
                    * start_offset_dist)[0],
            (quad_bezier_reverse.end
                + vector2_unit_norm(quad_bezier_reverse.end - quad_bezier_reverse.cp1)
                    * start_offset_dist)[1],
        )),
    ));

    commands
}

pub fn cubic_bezier_offsetted(
    cubic_bezier: CubicBezier,
    start_offset_dist: f64,
    end_offset_dist: f64,
    move_start: bool,
) -> Vec<path::Command> {
    let mid_offset_dist = (start_offset_dist + end_offset_dist) / 2.0;
    let (first_quad_bezier, second_quad_bezier) = split_cubic_bezier(cubic_bezier);

    let mut commands = quad_bezier_offsetted(
        first_quad_bezier,
        start_offset_dist,
        mid_offset_dist,
        move_start,
    );
    commands.append(&mut quad_bezier_offsetted(
        second_quad_bezier,
        mid_offset_dist,
        end_offset_dist,
        false,
    ));

    commands
}

pub fn split_cubic_bezier(cubic_bezier: CubicBezier) -> (QuadBezier, QuadBezier) {
    let cp_first = 0.25 * cubic_bezier.start + 0.75 * cubic_bezier.cp1;
    let cp_second = 0.25 * cubic_bezier.end + 0.75 * cubic_bezier.cp2;
    let mid = 0.5 * cp_first + 0.5 * cp_second;

    let first_quad_bezier = QuadBezier {
        start: cubic_bezier.start,
        cp1: cp_first,
        end: mid,
    };
    let second_quad_bezier = QuadBezier {
        start: mid,
        cp1: cp_second,
        end: cubic_bezier.end,
    };

    (first_quad_bezier, second_quad_bezier)
}

pub fn cubic_bezier_variable_width(
    cubic_bezier: CubicBezier,
    width_start: f64,
    width_end: f64,
) -> Vec<path::Command> {
    let (first_quad_bezier, second_quad_bezier) = split_cubic_bezier(cubic_bezier);

    let mut commands = Vec::new();
    commands.append(&mut quad_bezier_variable_width(
        first_quad_bezier,
        width_start,
        (width_start + width_end) / 2.0,
    ));
    commands.append(&mut quad_bezier_variable_width(
        second_quad_bezier,
        (width_start + width_end) / 2.0,
        width_end,
    ));

    commands
}

// Is None when one of the length is zero.
// See 'Conversion between Cubic Bezier Curves and Catmull-Rom Splines'
pub fn cubic_bezier_w_catmull_rom(
    first: &Element,
    second: &Element,
    third: &Element,
    forth: &Element,
) -> Option<CubicBezier> {
    /*     let catmullrom_spline = splines::spline::Spline::from_vec(vec![
        splines::key::Key::new(0.0, first.inputdata.pos(), splines::Interpolation::CatmullRom),
        splines::key::Key::new(0.0, second.inputdata.pos(), splines::Interpolation::CatmullRom),
        splines::key::Key::new(0.0, third.inputdata.pos(), splines::Interpolation::CatmullRom),
        splines::key::Key::new(1.0, forth.inputdata.pos(), splines::Interpolation::CatmullRom),
    ]); */

    let _second_third_len = (second.inputdata.pos() - third.inputdata.pos()).magnitude();

    // returns early when no length.
    /*     if second_third_len == 0.0 {
        return None;
    } */

    // Tension factor (tau), minimum 1.0
    let tension = 1.0;

    //
    let start = second.inputdata.pos();
    let cp1 =
        second.inputdata.pos() + (third.inputdata.pos() - first.inputdata.pos()) / (6.0 * tension);
    let cp2 =
        third.inputdata.pos() - (forth.inputdata.pos() - second.inputdata.pos()) / (6.0 * tension);
    let end = third.inputdata.pos();

    let mut cubic_bezier = CubicBezier {
        start,
        cp1,
        cp2,
        end,
    };

    // Avoiding curve loops and general line weirdness
    // TODO implement see https://pomax.github.io/bezierinfo/#canonical
    let start_cp1_len = (cp1 - start).magnitude();
    let start_cp2_len = (cp1 - start).magnitude();

    if start_cp1_len > (start_cp2_len + 1.0) {
        cubic_bezier.cp1 = cubic_bezier.start + (cp1 - start) * (start_cp2_len / start_cp1_len);
    }

    Some(cubic_bezier)
}

pub fn filter_prepare_line_from_input(
    first: &Element,
    second: &Element,
    offset: na::Vector2<f64>,
) -> Option<Line> {
    let line = Line {
        start: first.inputdata.pos() + offset,
        end: second.inputdata.pos() + offset,
    };

    let start_end_len = (line.end - line.start).magnitude();

    // returns early to prevent NaN when calculating the vector norm.
    if start_end_len == 0.0 {
        return None;
    }

    Some(line)
}
