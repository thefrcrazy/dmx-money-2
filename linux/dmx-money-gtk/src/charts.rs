//! Graphiques dessinés avec cairo : mêmes formes et mêmes couleurs que sur macOS et Windows.

use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{cairo, DrawingArea};

#[derive(Clone, Debug, Default)]
pub struct Series {
    pub name: String,
    pub color: String,
    pub values: Vec<f64>,
    pub dashed: bool,
    pub filled: bool,
}

#[derive(Clone, Debug)]
pub struct Reference {
    pub value: f64,
    pub color: String,
    pub dashed: bool,
}

#[derive(Clone, Debug)]
pub struct Marker {
    pub index: usize,
    pub color: String,
}

#[derive(Clone, Debug, Default)]
pub struct LineChartData {
    pub series: Vec<Series>,
    pub labels: Vec<String>,
    pub references: Vec<Reference>,
    pub markers: Vec<Marker>,
    pub stepped: bool,
}

#[derive(Clone, Debug, Default)]
pub struct BarChartData {
    pub labels: Vec<String>,
    /// Une valeur par série et par groupe.
    pub groups: Vec<Vec<f64>>,
    pub colors: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub struct DonutSlice {
    pub value: f64,
    pub color: String,
    pub hidden: bool,
}

#[derive(Clone, Debug, Default)]
pub struct DonutData {
    pub slices: Vec<DonutSlice>,
    pub center_title: String,
    pub center_value: String,
}

const AXIS_WIDTH: f64 = 46.0;
const LABEL_HEIGHT: f64 = 20.0;

fn rgb(hex: &str) -> (f64, f64, f64) {
    let clean: String = hex
        .trim_start_matches('#')
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .collect();
    let clean = if clean.len() == 3 {
        clean.chars().flat_map(|c| [c, c]).collect::<String>()
    } else {
        clean
    };
    if clean.len() < 6 {
        return (0.6, 0.6, 0.6);
    }
    let value = u32::from_str_radix(&clean[..6], 16).unwrap_or(0x9ca3af);
    (
        ((value >> 16) & 0xff) as f64 / 255.0,
        ((value >> 8) & 0xff) as f64 / 255.0,
        (value & 0xff) as f64 / 255.0,
    )
}

/// Graduations « rondes » d'un axe.
pub fn nice_ticks(minimum: f64, maximum: f64, count: usize) -> Vec<f64> {
    let (mut low, mut high) = (minimum, maximum);
    if (high - low).abs() < f64::EPSILON {
        low -= 1.0;
        high += 1.0;
    }
    let raw_step = (high - low) / (count.max(2) - 1) as f64;
    let magnitude = 10f64.powf(raw_step.abs().log10().floor());
    let residual = raw_step / magnitude;
    let step = if residual > 5.0 {
        10.0 * magnitude
    } else if residual > 2.0 {
        5.0 * magnitude
    } else if residual > 1.0 {
        2.0 * magnitude
    } else {
        magnitude
    };
    let start = (low / step).floor() * step;
    let end = (high / step).ceil() * step;
    let mut ticks = Vec::new();
    let mut value = start;
    while value <= end + step * 0.5 {
        ticks.push(value);
        value += step;
    }
    ticks
}

pub fn compact_label(value: f64) -> String {
    let absolute = value.abs();
    if absolute >= 1_000_000.0 {
        format!("{:.1}M", value / 1_000_000.0).replace('.', ",")
    } else if absolute >= 10_000.0 {
        format!("{:.0}k", value / 1_000.0)
    } else if absolute >= 1_000.0 {
        format!("{:.1}k", value / 1_000.0).replace('.', ",")
    } else {
        format!("{value:.0}")
    }
}

fn text_color(area: &DrawingArea) -> (f64, f64, f64) {
    let color = area.color();
    (color.red() as f64, color.green() as f64, color.blue() as f64)
}

/// Courbes (aires, escaliers, pointillés) avec seuils et marqueurs.
pub struct LineChart {
    area: DrawingArea,
    data: Rc<RefCell<LineChartData>>,
}

impl LineChart {
    pub fn new(height: i32) -> Self {
        let area = DrawingArea::new();
        area.set_content_height(height);
        area.set_hexpand(true);
        let data = Rc::new(RefCell::new(LineChartData::default()));
        let draw_data = data.clone();
        area.set_draw_func(move |area, context, width, height| {
            draw_line_chart(area, context, width as f64, height as f64, &draw_data.borrow());
        });
        Self { area, data }
    }

    pub fn widget(&self) -> &DrawingArea {
        &self.area
    }

    pub fn set_data(&self, data: LineChartData) {
        // Légende du graphique : GTK n'affiche pas les noms de séries, on les met en infobulle.
        let names: Vec<&str> = data
            .series
            .iter()
            .filter(|series| !series.name.is_empty())
            .map(|series| series.name.as_str())
            .collect();
        self.area
            .set_tooltip_text((!names.is_empty()).then(|| names.join(" · ")).as_deref());
        *self.data.borrow_mut() = data;
        self.area.queue_draw();
    }
}

fn draw_line_chart(area: &DrawingArea, context: &cairo::Context, width: f64, height: f64, data: &LineChartData) {
    let count = data.series.iter().map(|series| series.values.len()).max().unwrap_or(0);
    if count == 0 {
        return;
    }
    let plot_x = AXIS_WIDTH;
    let plot_y = 6.0;
    let plot_width = (width - AXIS_WIDTH - 10.0).max(1.0);
    let plot_height = (height - LABEL_HEIGHT - 14.0).max(1.0);
    let plot_bottom = plot_y + plot_height;

    let mut values: Vec<f64> = data.series.iter().flat_map(|series| series.values.clone()).collect();
    values.extend(data.references.iter().map(|reference| reference.value));
    let minimum = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let maximum = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let ticks = nice_ticks(minimum, maximum, 5);
    let low = *ticks.first().unwrap_or(&0.0);
    let high = *ticks.last().unwrap_or(&1.0);
    let span = if (high - low).abs() < f64::EPSILON {
        1.0
    } else {
        high - low
    };

    let x_at = |index: usize| -> f64 {
        if count <= 1 {
            plot_x + plot_width / 2.0
        } else {
            plot_x + plot_width * index as f64 / (count - 1) as f64
        }
    };
    let y_at = |value: f64| -> f64 { plot_bottom - plot_height * (value - low) / span };

    let (text_r, text_g, text_b) = text_color(area);
    context.set_line_width(1.0);
    context.select_font_face("Cantarell", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
    context.set_font_size(10.0);

    for tick in &ticks {
        let y = y_at(*tick);
        context.set_source_rgba(text_r, text_g, text_b, 0.12);
        context.set_dash(&[3.0, 3.0], 0.0);
        context.move_to(plot_x, y);
        context.line_to(plot_x + plot_width, y);
        let _ = context.stroke();
        context.set_dash(&[], 0.0);

        let label = compact_label(*tick);
        context.set_source_rgba(text_r, text_g, text_b, 0.6);
        if let Ok(extents) = context.text_extents(&label) {
            context.move_to(AXIS_WIDTH - 8.0 - extents.width(), y + 3.0);
            let _ = context.show_text(&label);
        }
    }

    for marker in &data.markers {
        if marker.index >= count {
            continue;
        }
        let (r, g, b) = rgb(&marker.color);
        context.set_source_rgba(r, g, b, 0.85);
        context.set_line_width(2.0);
        context.set_dash(&[3.0, 3.0], 0.0);
        context.move_to(x_at(marker.index), plot_y);
        context.line_to(x_at(marker.index), plot_bottom);
        let _ = context.stroke();
        context.set_dash(&[], 0.0);
    }

    for reference in &data.references {
        let (r, g, b) = rgb(&reference.color);
        context.set_source_rgb(r, g, b);
        context.set_line_width(1.0);
        if reference.dashed {
            context.set_dash(&[6.0, 4.0], 0.0);
        }
        let y = y_at(reference.value);
        context.move_to(plot_x, y);
        context.line_to(plot_x + plot_width, y);
        let _ = context.stroke();
        context.set_dash(&[], 0.0);
    }

    for series in &data.series {
        if series.values.is_empty() {
            continue;
        }
        let (r, g, b) = rgb(&series.color);
        let trace = |context: &cairo::Context| {
            for (index, value) in series.values.iter().enumerate() {
                let x = x_at(index);
                if index == 0 {
                    context.move_to(x, y_at(*value));
                } else {
                    if data.stepped {
                        context.line_to(x, y_at(series.values[index - 1]));
                    }
                    context.line_to(x, y_at(*value));
                }
            }
        };

        if series.filled {
            trace(context);
            context.line_to(x_at(series.values.len() - 1), plot_bottom);
            context.line_to(x_at(0), plot_bottom);
            context.close_path();
            let gradient = cairo::LinearGradient::new(0.0, plot_y, 0.0, plot_bottom);
            gradient.add_color_stop_rgba(0.0, r, g, b, 0.35);
            gradient.add_color_stop_rgba(1.0, r, g, b, 0.0);
            let _ = context.set_source(&gradient);
            let _ = context.fill();
        }

        context.set_source_rgb(r, g, b);
        context.set_line_width(if series.dashed { 1.5 } else { 2.0 });
        if series.dashed {
            context.set_dash(&[4.0, 3.0], 0.0);
        }
        trace(context);
        let _ = context.stroke();
        context.set_dash(&[], 0.0);
    }

    let step = ((count as f64 / (plot_width / 80.0).max(2.0)).ceil() as usize).max(1);
    context.set_source_rgba(text_r, text_g, text_b, 0.6);
    for index in (0..count).step_by(step) {
        if let Some(label) = data.labels.get(index) {
            if let Ok(extents) = context.text_extents(label) {
                context.move_to(x_at(index) - extents.width() / 2.0, plot_bottom + 14.0);
                let _ = context.show_text(label);
            }
        }
    }
}

/// Barres groupées (revenus contre dépenses).
pub struct BarChart {
    area: DrawingArea,
    data: Rc<RefCell<BarChartData>>,
}

impl BarChart {
    pub fn new(height: i32) -> Self {
        let area = DrawingArea::new();
        area.set_content_height(height);
        area.set_hexpand(true);
        let data = Rc::new(RefCell::new(BarChartData::default()));
        let draw_data = data.clone();
        area.set_draw_func(move |area, context, width, height| {
            draw_bar_chart(area, context, width as f64, height as f64, &draw_data.borrow());
        });
        Self { area, data }
    }

    pub fn widget(&self) -> &DrawingArea {
        &self.area
    }

    pub fn set_data(&self, data: BarChartData) {
        *self.data.borrow_mut() = data;
        self.area.queue_draw();
    }
}

fn draw_bar_chart(area: &DrawingArea, context: &cairo::Context, width: f64, height: f64, data: &BarChartData) {
    if data.groups.is_empty() {
        return;
    }
    let plot_x = AXIS_WIDTH;
    let plot_y = 6.0;
    let plot_width = (width - AXIS_WIDTH - 10.0).max(1.0);
    let plot_height = (height - LABEL_HEIGHT - 14.0).max(1.0);
    let plot_bottom = plot_y + plot_height;

    let maximum = data
        .groups
        .iter()
        .flat_map(|group| group.iter().cloned())
        .fold(0.0f64, f64::max)
        .max(1.0);
    let ticks = nice_ticks(0.0, maximum, 5);
    let high = *ticks.last().unwrap_or(&1.0);
    let slot = plot_width / data.groups.len() as f64;
    let series_count = data.groups[0].len().max(1);
    let bar_width = (slot * 0.8 / series_count as f64).clamp(2.0, 28.0);

    let (text_r, text_g, text_b) = text_color(area);
    context.set_font_size(10.0);

    for tick in &ticks {
        let y = plot_bottom - plot_height * tick / high;
        context.set_source_rgba(text_r, text_g, text_b, 0.12);
        context.set_dash(&[3.0, 3.0], 0.0);
        context.move_to(plot_x, y);
        context.line_to(plot_x + plot_width, y);
        let _ = context.stroke();
        context.set_dash(&[], 0.0);
        let label = compact_label(*tick);
        context.set_source_rgba(text_r, text_g, text_b, 0.6);
        if let Ok(extents) = context.text_extents(&label) {
            context.move_to(AXIS_WIDTH - 8.0 - extents.width(), y + 3.0);
            let _ = context.show_text(&label);
        }
    }

    let label_step = ((data.groups.len() as f64 / (plot_width / 70.0).max(1.0)).ceil() as usize).max(1);
    for (index, group) in data.groups.iter().enumerate() {
        let group_start = plot_x + slot * index as f64 + (slot - bar_width * series_count as f64) / 2.0;
        for (series, value) in group.iter().enumerate() {
            let bar_height = plot_height * value / high;
            let (r, g, b) = rgb(data.colors.get(series).map(String::as_str).unwrap_or("#6366f1"));
            context.set_source_rgb(r, g, b);
            rounded_rect(
                context,
                group_start + bar_width * series as f64,
                plot_bottom - bar_height.max(0.0),
                bar_width,
                bar_height.max(0.0),
                4.0,
            );
            let _ = context.fill();
        }
        if index % label_step == 0 {
            if let Some(label) = data.labels.get(index) {
                context.set_source_rgba(text_r, text_g, text_b, 0.6);
                if let Ok(extents) = context.text_extents(label) {
                    context.move_to(
                        plot_x + slot * index as f64 + slot / 2.0 - extents.width() / 2.0,
                        plot_bottom + 14.0,
                    );
                    let _ = context.show_text(label);
                }
            }
        }
    }
}

fn rounded_rect(context: &cairo::Context, x: f64, y: f64, width: f64, height: f64, radius: f64) {
    let radius = radius.min(width / 2.0).min(height / 2.0).max(0.0);
    context.new_sub_path();
    context.arc(
        x + width - radius,
        y + radius,
        radius,
        -std::f64::consts::FRAC_PI_2,
        0.0,
    );
    context.arc(
        x + width - radius,
        y + height - radius,
        radius,
        0.0,
        std::f64::consts::FRAC_PI_2,
    );
    context.arc(
        x + radius,
        y + height - radius,
        radius,
        std::f64::consts::FRAC_PI_2,
        std::f64::consts::PI,
    );
    context.arc(
        x + radius,
        y + radius,
        radius,
        std::f64::consts::PI,
        1.5 * std::f64::consts::PI,
    );
    context.close_path();
}

/// Anneau de répartition ; les parts masquées ne comptent pas dans le total.
pub struct DonutChart {
    area: DrawingArea,
    data: Rc<RefCell<DonutData>>,
}

impl DonutChart {
    pub fn new(height: i32, thickness: f64) -> Self {
        let area = DrawingArea::new();
        area.set_content_height(height);
        area.set_hexpand(true);
        let data = Rc::new(RefCell::new(DonutData::default()));
        let draw_data = data.clone();
        area.set_draw_func(move |area, context, width, height| {
            draw_donut(
                area,
                context,
                width as f64,
                height as f64,
                &draw_data.borrow(),
                thickness,
            );
        });
        Self { area, data }
    }

    pub fn widget(&self) -> &DrawingArea {
        &self.area
    }

    pub fn set_data(&self, data: DonutData) {
        *self.data.borrow_mut() = data;
        self.area.queue_draw();
    }
}

fn draw_donut(area: &DrawingArea, context: &cairo::Context, width: f64, height: f64, data: &DonutData, thickness: f64) {
    let size = width.min(height);
    let center_x = width / 2.0;
    let center_y = height / 2.0;
    let radius = (size - thickness) / 2.0;
    if radius <= 0.0 {
        return;
    }
    let (text_r, text_g, text_b) = text_color(area);

    context.set_line_width(thickness);
    context.set_source_rgba(text_r, text_g, text_b, 0.08);
    context.arc(center_x, center_y, radius, 0.0, std::f64::consts::TAU);
    let _ = context.stroke();

    let visible: Vec<&DonutSlice> = data
        .slices
        .iter()
        .filter(|slice| !slice.hidden && slice.value > 0.0)
        .collect();
    let total: f64 = visible.iter().map(|slice| slice.value).sum();
    if total > 0.0 {
        let gap = if visible.len() > 1 { 0.004 } else { 0.0 };
        let mut cursor = 0.0;
        for slice in visible {
            let start = cursor + gap;
            cursor += slice.value / total;
            let end = (cursor - gap).max(start + 0.0005);
            let (r, g, b) = rgb(&slice.color);
            context.set_source_rgb(r, g, b);
            context.arc(
                center_x,
                center_y,
                radius,
                start * std::f64::consts::TAU - std::f64::consts::FRAC_PI_2,
                end * std::f64::consts::TAU - std::f64::consts::FRAC_PI_2,
            );
            let _ = context.stroke();
        }
    }

    context.set_source_rgba(text_r, text_g, text_b, 0.65);
    context.set_font_size(10.0);
    let title = data.center_title.to_uppercase();
    if let Ok(extents) = context.text_extents(&title) {
        context.move_to(center_x - extents.width() / 2.0, center_y - 4.0);
        let _ = context.show_text(&title);
    }
    context.set_source_rgb(text_r, text_g, text_b);
    context.select_font_face("Cantarell", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
    context.set_font_size(15.0);
    if let Ok(extents) = context.text_extents(&data.center_value) {
        context.move_to(center_x - extents.width() / 2.0, center_y + 14.0);
        let _ = context.show_text(&data.center_value);
    }
    context.select_font_face("Cantarell", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
}
