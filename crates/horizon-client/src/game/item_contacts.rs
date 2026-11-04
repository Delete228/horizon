//! Owned item collision callbacks; no entity lookup or deferred contact queue.
use super::entity::Entity as _;
use super::{Game, ItemEntity};
use horizon_client::join_seq::JoinList;
use horizon_core::geometry::Aabb;

impl Game<'_> {
    /// The ordinary item tick with a deliberate block-contact extension between
    /// collision response and block effects. `client` describes prediction, not
    /// authority to change a server inventory.
    pub(super) fn tick_item_with_block_contacts(
        &mut self,
        item: &mut ItemEntity,
        no_physics: bool,
        entity_boxes: &[Aabb],
        client: bool,
    ) -> (bool, bool) {
        if horizon_client::mod_api::mods().is_empty() {
            return (
                super::entity::tick_item(
                    item,
                    &self.world,
                    self.pack.assets.registry,
                    self.res.facts(),
                    self.collision,
                    &self.res.motion().fluids,
                    no_physics,
                    entity_boxes,
                    Some(self.border.geometry()),
                ),
                false,
            );
        }
        if item.stack.count == 0 {
            return (false, true);
        }
        let heights = item.push_by_fluid(&self.world, &self.res.motion().fluids);
        let step = item.tick_move_before_block_effects(
            &self.world,
            self.res.facts(),
            self.collision,
            no_physics,
            heights,
            entity_boxes,
            Some(self.border.geometry()),
            Some(&self.res.motion().blocks),
        );
        let mut discard = false;
        if let Some(pos) = step.vertical_contact {
            let state = self.world.block(pos.x, pos.y, pos.z);
            self.native_item_after_vertical_collision(
                self.dim.id(),
                client,
                item.id,
                state,
                [pos.x, pos.y, pos.z],
                [item.position.x, item.position.y, item.position.z],
                &mut item.stack,
                &mut discard,
            );
            discard |= item.stack.count == 0;
        }
        // Discard does not undo collision response or skip the rest of move.
        // The owner omits merging, pickup and destruction of this item afterwards.
        item.finish_move_block_effects(&self.world, self.res.facts(), step);
        item.push_by_bubble_column(
            &self.world,
            self.pack.assets.registry,
            self.res.facts(),
            self.collision,
        );
        let alive = item.tick_settle(&self.world, self.res.facts(), no_physics, step);
        item.push_by_fluid(&self.world, &self.res.motion().fluids);
        (alive, discard)
    }
    /// A callback may spawn another item while the active batch is borrowed out.
    /// Keep those newer joins after this batch, unticked until the next tick.
    pub(super) fn restore_item_batch(&mut self, mut items: JoinList<ItemEntity>) {
        items.extend(std::mem::take(&mut self.dropped));
        self.dropped = items;
    }
}
