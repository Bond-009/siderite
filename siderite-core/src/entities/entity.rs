bitflags! {
    #[derive(Default)]
    pub struct Tmp: u8 {
        const ON_FIRE = 0x01;
        const CROUCHED = 0x02;
        const SPRINTING = 0x04;

        /// Eating/Drinking/Blocking
        const EATING = 0x10;

        const INVISABLE = 0x20;
    }
}

pub struct Entity {
    air: i16,
    name_tag: Option<String>,
    always_show_name_tag: bool,
    silent: bool
}

pub struct LivingEntityBase {
    entity: Entity,
    health: f32,
    potion_color: i32,
    is_potion_effect_ambient: bool,
    number_of_arrows_in_entity: i8
}

pub struct LivingEntity {
    living_entity_base: LivingEntityBase,
    ai_disaled: bool
}
