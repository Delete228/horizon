package net.createmod.catnip.platform;

import net.createmod.catnip.platform.services.ModFluidHelper;

// Oracle dependency fixture only. The original FluidRenderHelper bytecode reads this service;
// the test supplies explicit sprite/color/luminosity without booting a mod loader or game.
public final class CatnipServices {
    public static ModFluidHelper<Object> FLUID_HELPER;
}
