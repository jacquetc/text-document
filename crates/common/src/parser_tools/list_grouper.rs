use crate::entities::ListStyle;
use crate::types::EntityId;

/// Tracks active list entities across consecutive blocks so that items
/// belonging to the same logical list share a single list entity.
///
/// Uses a vec indexed by indent level. When indent decreases, deeper
/// entries are truncated so that outer lists resume correctly.
pub struct ListGrouper {
    /// Index = indent level. Each entry: (entity_id, style).
    active: Vec<Option<(EntityId, ListStyle)>>,
}

impl ListGrouper {
    pub fn new() -> Self {
        Self { active: Vec::new() }
    }

    /// Returns an existing list entity id if the style and indent match
    /// a previously registered list at this level. Returns `None` if a
    /// new list entity must be created (caller should then call `register`).
    pub fn try_reuse(&mut self, style: &ListStyle, indent: u32) -> Option<EntityId> {
        let idx = indent as usize;
        // Truncate deeper levels - we returned to a shallower depth
        self.active.truncate(idx + 1);
        if let Some(Some((id, existing_style))) = self.active.get(idx) {
            if existing_style == style {
                return Some(*id);
            }
        }
        None
    }

    /// Register a newly created list entity at the given indent level.
    pub fn register(&mut self, id: EntityId, style: ListStyle, indent: u32) {
        let idx = indent as usize;
        while self.active.len() <= idx {
            self.active.push(None);
        }
        self.active[idx] = Some((id, style));
    }

    /// Clear all tracking. Call on non-list blocks, tables, or frame boundaries.
    pub fn reset(&mut self) {
        self.active.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_item_returns_none() {
        let mut g = ListGrouper::new();
        assert!(g.try_reuse(&ListStyle::Decimal, 0).is_none());
    }

    #[test]
    fn consecutive_same_style_reuses() {
        let mut g = ListGrouper::new();
        g.register(42, ListStyle::Decimal, 0);
        assert_eq!(g.try_reuse(&ListStyle::Decimal, 0), Some(42));
    }

    #[test]
    fn different_style_creates_new() {
        let mut g = ListGrouper::new();
        g.register(42, ListStyle::Decimal, 0);
        assert!(g.try_reuse(&ListStyle::Disc, 0).is_none());
    }

    #[test]
    fn different_indent_creates_new() {
        let mut g = ListGrouper::new();
        g.register(42, ListStyle::Decimal, 0);
        assert!(g.try_reuse(&ListStyle::Decimal, 1).is_none());
    }

    #[test]
    fn reset_clears_all() {
        let mut g = ListGrouper::new();
        g.register(42, ListStyle::Decimal, 0);
        g.reset();
        assert!(g.try_reuse(&ListStyle::Decimal, 0).is_none());
    }

    #[test]
    fn nested_indent_resumes_outer() {
        let mut g = ListGrouper::new();
        g.register(10, ListStyle::Decimal, 0);
        g.register(20, ListStyle::LowerAlpha, 1);
        // Return to indent 0 - should resume outer list
        assert_eq!(g.try_reuse(&ListStyle::Decimal, 0), Some(10));
    }

    #[test]
    fn nested_indent_different_style_creates_new() {
        let mut g = ListGrouper::new();
        g.register(10, ListStyle::Decimal, 0);
        g.register(20, ListStyle::LowerAlpha, 1);
        // Return to indent 0 with different style
        assert!(g.try_reuse(&ListStyle::Disc, 0).is_none());
    }
}
