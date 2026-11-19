use patina_sdk::node;

/// A bus of 8 bits with 8 drivers, in place of the tri-state buffers of Digital Logic Sim 2. A
/// wire has one driver, so each driver connects its value `dN` and its enable `onN` here. The bus
/// gives the value of the driver that is on. Two drivers that are on give the OR of their values,
/// and no driver gives 0.
#[node(color = "#175c59")]
#[allow(clippy::too_many_arguments)]
pub fn bus(d0: &u8, on0: &bool, d1: &u8, on1: &bool, d2: &u8, on2: &bool, d3: &u8, on3: &bool, d4: &u8, on4: &bool, d5: &u8, on5: &bool, d6: &u8, on6: &bool, d7: &u8, on7: &bool) -> u8 {
    let drivers = [(d0, on0), (d1, on1), (d2, on2), (d3, on3), (d4, on4), (d5, on5), (d6, on6), (d7, on7)];
    drivers.iter().filter(|(_, on)| **on).fold(0, |bus, (d, _)| bus | **d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gives_the_drivers_that_are_on() {
        let (t, f) = (&true, &false);
        assert_eq!(bus(&7, f, &0x30, t, &9, f, &1, f, &2, f, &3, f, &4, f, &5, f), 0x30);
        assert_eq!(bus(&7, f, &0x30, t, &9, f, &1, f, &2, f, &3, f, &4, f, &0x05, t), 0x35);
        assert_eq!(bus(&7, f, &8, f, &9, f, &1, f, &2, f, &3, f, &4, f, &5, f), 0);
    }
}
