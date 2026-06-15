use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

use crate::ui::style::theme;

/// Renders a placeholder line inside a panel's inner area.
///
/// Standardizes empty/loading states across every list panel: loading uses the
/// primary color with a `◌` marker, empty/info states use muted gray. Always
/// drawn at the same position so panels look consistent.
pub fn render_placeholder(buf: &mut Buffer, inner: Rect, msg: &str, loading: bool) {
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let (style, text) = if loading {
        (
            Style::default().fg(theme::color_primary()),
            format!("◌ {msg}"),
        )
    } else {
        (Style::default().fg(theme::color_muted()), msg.to_string())
    };
    buf.set_string(inner.x + 1, inner.y, &text, style);
}

pub mod buckets;
pub mod clusters;
pub mod containers;
pub mod detail;
pub mod instances;
pub mod log_groups;
pub mod log_streams;
pub mod log_viewer;
pub mod objects;
pub mod output;
pub mod query_results;
pub mod rds_instances;
pub mod rds_tables;
pub mod services;
pub mod tasks;
pub mod terminal;

pub use buckets::BucketsPanel;
pub use clusters::ClustersPanel;
pub use containers::ContainersPanel;
pub use detail::DetailPanel;
pub use instances::InstancesPanel;
pub use log_groups::LogGroupsPanel;
pub use log_streams::LogStreamsPanel;
pub use log_viewer::{render_log_detail, LogViewerPanel};
pub use objects::{ObjectsPanel, S3ObjectItem};
pub use output::OutputPanel;
pub use query_results::QueryResultsPanel;
pub use rds_instances::RdsInstancesPanel;
pub use rds_tables::RdsTablesPanel;
pub use services::ServicesPanel;
pub use tasks::TasksPanel;
pub use terminal::TerminalPanel;
