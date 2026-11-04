//! Component-bearing values through the generated capability dispatch boundary.
use horizon_client::mod_api::{
    Compound, CustomFluidDrainEvent, CustomFluidFillEvent, CustomFluidTanksEvent, Hooks, Host,
    NativeFluidAction, NativeFluidDrain, NativeFluidStack, NativeFluidTankView, Tag,
    dispatch_custom_fluid_drain, dispatch_custom_fluid_fill, dispatch_custom_fluid_tanks,
};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, PartialEq)]
struct Saved {
    content: NativeFluidStack,
    inventory: Vec<Tag>,
    dirty: u32,
    sync: u32,
    random_draws: u32,
}
struct Provider(Arc<Mutex<Saved>>);
impl Hooks for Provider {
    fn custom_fluid_tanks(
        &self,
        _: &dyn Host,
        event: &mut CustomFluidTanksEvent,
    ) -> Option<Vec<NativeFluidTankView>> {
        assert_eq!(event.pos, [4, 65, 8]);
        assert!(event.side.is_none());
        let saved = self.0.lock().unwrap();
        Some(vec![NativeFluidTankView {
            tank: 0,
            content: (saved.content.amount > 0).then(|| saved.content.clone()),
            capacity: 1000,
            can_fill: true,
            can_drain: true,
        }])
    }
    fn custom_fluid_fill(
        &self,
        _: &mut dyn Host,
        event: &mut CustomFluidFillEvent<'_>,
    ) -> Option<i32> {
        let mut saved = self.0.lock().unwrap();
        let source = event.resource;
        if source.fluid != saved.content.fluid || source.components != saved.content.components {
            return Some(0);
        }
        let accepted = source.amount.min(1000 - saved.content.amount).max(0);
        if event.action == NativeFluidAction::Execute && accepted > 0 {
            saved.content.amount += accepted;
            saved.dirty += 1;
            saved.sync += 1;
        }
        Some(accepted)
    }
    fn custom_fluid_drain(
        &self,
        _: &mut dyn Host,
        event: &mut CustomFluidDrainEvent<'_>,
    ) -> Option<NativeFluidStack> {
        let mut saved = self.0.lock().unwrap();
        let maximum = match event.request {
            NativeFluidDrain::Any(maximum) => *maximum,
            NativeFluidDrain::Exact(selected)
                if selected.fluid == saved.content.fluid
                    && selected.components == saved.content.components =>
            {
                selected.amount
            }
            _ => return None,
        };
        let amount = maximum.min(saved.content.amount);
        if amount <= 0 {
            return None;
        }
        let mut result = saved.content.clone();
        result.amount = amount;
        if event.action == NativeFluidAction::Execute {
            saved.content.amount -= amount;
            saved.dirty += 1;
            saved.sync += 1;
        }
        Some(result)
    }
}
struct Caller;
impl Host for Caller {}

#[test]
fn simulation_preserves_storage_and_owned_payloads_keep_component_removals() {
    let components = Compound::from([
        ("sample:color".into(), Tag::Int(0x334455)),
        ("!sample:glow".into(), Tag::Compound(Compound::new())),
    ]);
    let storage = Arc::new(Mutex::new(Saved {
        content: NativeFluidStack {
            fluid: "sample:ink".into(),
            amount: 750,
            components,
        },
        inventory: vec![Tag::compound([("sample:item_marker", Tag::Long(991))])],
        dirty: 3,
        sync: 2,
        random_draws: 7,
    }));
    let mods: Vec<Box<dyn Hooks>> = vec![Box::new(Provider(storage.clone()))];
    let before = storage.lock().unwrap().clone();
    let mut fill = before.content.clone();
    fill.amount = 500;
    let mut accepted = None;
    dispatch_custom_fluid_fill(
        &mut Caller,
        &mods,
        [4, 65, 8],
        None,
        &fill,
        NativeFluidAction::Simulate,
        &mut accepted,
    );
    assert_eq!(accepted, Some(250));
    let mut drained = None;
    dispatch_custom_fluid_drain(
        &mut Caller,
        &mods,
        [4, 65, 8],
        None,
        &NativeFluidDrain::Any(100),
        NativeFluidAction::Simulate,
        &mut drained,
    );
    assert_eq!(drained.unwrap().amount, 100);
    let mut view = None;
    dispatch_custom_fluid_tanks(&Caller, &mods, [4, 65, 8], None, &mut view);
    let mut owned = view.unwrap().remove(0).content.unwrap();
    owned.components.clear();
    assert_eq!(*storage.lock().unwrap(), before);

    // A removed component differs from absence and may not select this fluid.
    let mut mismatched = fill.clone();
    mismatched.components.remove(&"!sample:glow".into());
    let mut refused = None;
    dispatch_custom_fluid_drain(
        &mut Caller,
        &mods,
        [4, 65, 8],
        None,
        &NativeFluidDrain::Exact(mismatched),
        NativeFluidAction::Execute,
        &mut refused,
    );
    assert!(refused.is_none());
    assert_eq!(*storage.lock().unwrap(), before);

    let mut accepted = None;
    dispatch_custom_fluid_fill(
        &mut Caller,
        &mods,
        [4, 65, 8],
        None,
        &fill,
        NativeFluidAction::Execute,
        &mut accepted,
    );
    assert_eq!(accepted, Some(250));
    let mut drained = None;
    dispatch_custom_fluid_drain(
        &mut Caller,
        &mods,
        [4, 65, 8],
        None,
        &NativeFluidDrain::Exact(fill),
        NativeFluidAction::Execute,
        &mut drained,
    );
    let result = drained.unwrap();
    assert_eq!(result.amount, 500);
    assert_eq!(result.components, before.content.components);
    let after = storage.lock().unwrap();
    assert_eq!(after.content.amount, 500);
    assert_eq!(after.inventory, before.inventory);
    assert_eq!((after.dirty, after.sync, after.random_draws), (5, 4, 7));
}
