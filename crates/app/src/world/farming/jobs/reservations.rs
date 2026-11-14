//! Claims allocate carried blocks to cells; they never create or debit stock.
use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Claim {
    pub worker: u32,
    pub order: Order,
}

impl State {
    pub(super) fn available(&self, order: Order) -> bool {
        !self.claims.iter().any(|c| c.order.at == order.at)
    }

    pub(super) fn owns(&self, order: Order) -> bool {
        self.claims.iter().any(|c| Some(c.worker) == self.worker && c.order == order)
    }

    pub(super) fn release_claims(&mut self) {
        self.claims.retain(|c| Some(c.worker) != self.worker);
    }

    pub(super) fn reserve(&mut self, orders: impl IntoIterator<Item = Order>, count: usize) {
        let Some(worker) = self.worker else { return };
        for order in orders.into_iter().take(count) {
            if self.available(order) { self.claims.push(Claim { worker, order }); }
        }
    }

    pub(super) fn reconcile_claims(&mut self) {
        // Completed or canceled cells release claims, but their cargo stays physical.
        self.claims = self.claims.iter().filter(|c| self.build.contains(&c.order)
            && crew::view(self,c.worker).is_some_and(|a|a.building_material
                && supply::matches_source(self,a.cargo,c.order))).cloned().collect();
        let own = self.claims.iter().filter(|c| Some(c.worker) == self.worker).count();
        let Some(stack) = self.carry.filter(|_| self.building_material) else {
            self.release_claims();
            return;
        };
        // Legacy saves have cargo but no claims. Allocate only unclaimed work.
        let missing = (stack.count as usize).saturating_sub(own);
        if missing == 0 { return; }
        let pending: Vec<_> = self.build.iter().copied()
            .filter(|o| o.material() == Some(stack.item) && self.available(*o)
                && supply::matches_source(self,self.cargo,*o)
                && (!self.growth_active() || self.growth_funded_order(*o))).take(missing).collect();
        self.reserve(pending, missing);
    }

    pub(super) fn claims_valid(&self) -> bool {
        self.claims.len() <= construction::LIMIT && self.claims.iter().enumerate().all(|(i, c)| {
            self.build.contains(&c.order)
                && !self.claims[..i].iter().any(|old| old.order.at == c.order.at)
                && crew::view(self, c.worker).is_some_and(|a| a.building_material
                    && supply::matches_source(self,a.cargo,c.order) && a.carry.is_some_and(|s| {
                    c.order.material() == Some(s.item)
                        && self.claims.iter().filter(|other| other.worker == c.worker).count() <= s.count as usize
                }))
        })
    }
}

impl State {
    pub(super) fn growth_active(&self) -> bool {
        self.quarter.as_ref().and_then(|q|q.frontier.as_ref()).and_then(|s|s.growth.as_ref())
            .is_some_and(|g|g.active_project().is_some())
    }
    pub(super) fn growth_funded_order(&self, order: Order) -> bool {
        if let Some(funded)=supply::funded(self,order) { return funded; }
        self.quarter.as_ref().and_then(|q|q.frontier.as_ref()).is_some_and(|s|
            s.growth.as_ref().and_then(|g|g.active_project())
                .and_then(|i|s.projects.get(i)).is_some_and(|p|p.cells.contains(&order)))
    }
    pub(super) fn growth_reserved(&self, w: &World, item: crate::world::inventory::Item) -> u32 {
        self.quarter.as_ref().and_then(|q|q.frontier.as_ref()).map_or(0,|s|
            s.growth.as_ref().and_then(|g|g.active_project())
                .and_then(|i|s.projects.get(i)).map_or(0,|p|quarter::frontier::growth_required(w,self,p,item)))
    }
    pub(super) fn unreserved_stock(&self, w: &World, pile: IVec3, item: crate::world::inventory::Item) -> u32 {
        self.unreserved_from(w,pile,item,production::count(&w.block_data.chest(pile),item))
    }
    pub(super) fn unreserved_from(&self, w: &World, pile: IVec3, item: crate::world::inventory::Item, stock: u32) -> u32 {
        stock.saturating_sub(self.reserved_at(w,pile,item))
    }
    pub(super) fn reserved_at(&self, w: &World, pile: IVec3, item: crate::world::inventory::Item) -> u32 {
        if let Some(amount) = supply::reserved(w,self,pile,item) { return amount; }
        let bound = self.quarter.as_ref().and_then(|q|q.frontier.as_ref()).and_then(|s|s.growth.as_ref())
            .is_some_and(|g|g.stockpile == pile.to_array());
        if bound {self.growth_reserved(w,item)} else {0}
    }
    pub(super) fn build_stock(&self, w: &World, pile: IVec3, order: Order) -> u32 {
        if !known(w,pile).is_some_and(|b|b.chest_tier().is_some()) {return 0;}
        self.build_stock_from(w,pile,order,&w.block_data.chest(pile))
    }
    pub(super) fn build_stock_from(&self, w: &World, pile: IVec3, order: Order, slots: &[Option<ItemStack>]) -> u32 {
        let Some(item) = order.material() else {return 0;};
        if supply::source(w,self,order) != Some(pile) { return 0; }
        if supply::remote(self,order) {
            return if self.growth_funded_order(order) {production::count(slots,item)} else {0};
        }
        if self.growth_funded_order(order) {
            let bound = self.quarter.as_ref().and_then(|q|q.frontier.as_ref()).and_then(|s|s.growth.as_ref())
                .is_some_and(|g|g.stockpile == pile.to_array());
            if bound {production::count(slots,item)} else {0}
        } else { self.unreserved_from(w,pile,item,production::count(slots,item)) }
    }
}

impl State {
    pub(super) fn growth_ingredients_available(&self, w: &World, next: &[Option<ItemStack>], recipe: &crate::world::inventory::CraftingRecipe) -> bool {
        self.stockpile.is_none_or(|pile|self.ingredients_available_at(w,pile.into(),next,recipe))
    }
    pub(super) fn ingredients_available_at(&self, w: &World, pile: IVec3, next: &[Option<ItemStack>], recipe: &crate::world::inventory::CraftingRecipe) -> bool {
        recipe.ingredients.iter().all(|(item,_)|production::count(next,*item) >= self.reserved_at(w,pile,*item))
    }
}
