// Development-only collector: executes the player's own Catnip 1.0.82 method unchanged.
import com.google.gson.*;
import com.mojang.blaze3d.vertex.PoseStack;
import com.mojang.blaze3d.vertex.VertexConsumer;
import net.createmod.catnip.render.FluidRenderHelper;
import net.createmod.catnip.platform.CatnipServices;
import net.createmod.catnip.platform.services.ModFluidHelper;
import net.minecraft.client.renderer.texture.SpriteContents;
import net.minecraft.client.renderer.texture.TextureAtlasSprite;
import net.minecraft.world.level.BlockAndTintGetter;
import net.minecraft.core.BlockPos;
import net.minecraft.world.level.material.FluidState;
import net.minecraft.resources.ResourceLocation;
import sun.misc.Unsafe;
import java.io.*;
import java.lang.reflect.*;
import java.util.*;

public final class NativeFluidRenderOracle {
    static final Gson JSON = new Gson();
    static final class Sprite extends TextureAtlasSprite {
        Sprite(SpriteContents contents, int side) {
            super(ResourceLocation.fromNamespaceAndPath("minecraft", "textures/atlas/blocks.png"), contents, side, side, 0, 0);
        }
    }
    static Sprite sprite(int side) throws Exception {
        Field field = Unsafe.class.getDeclaredField("theUnsafe"); field.setAccessible(true);
        Unsafe unsafe = (Unsafe)field.get(null);
        SpriteContents contents = (SpriteContents)unsafe.allocateInstance(SpriteContents.class);
        for (String name : new String[]{"width", "height"}) {
            Field f = SpriteContents.class.getDeclaredField(name); f.setAccessible(true); f.setInt(contents, side);
        }
        return new Sprite(contents, side);
    }
    static final class Collector implements VertexConsumer {
        final List<Map<String,Object>> vertices = new ArrayList<>();
        Map<String,Object> current;
        public VertexConsumer addVertex(float x,float y,float z) {
            current = new LinkedHashMap<>(); current.put("pos",new float[]{x,y,z}); vertices.add(current); return this;
        }
        public VertexConsumer setColor(int r,int g,int b,int a) { current.put("rgba",new int[]{r,g,b,a}); return this; }
        public VertexConsumer setUv(float u,float v) { current.put("uv",new float[]{u,v}); return this; }
        public VertexConsumer setUv1(int u,int v) { return this; }
        public VertexConsumer setUv2(int u,int v) { current.put("light",u|(v<<16)); return this; }
        public VertexConsumer setNormal(float x,float y,float z) { current.put("normal",new float[]{x,y,z}); return this; }
    }
    static Object run(JsonObject request) throws Exception {
        float[] b=JSON.fromJson(request.get("bounds"),float[].class);
        int side=request.get("atlas").getAsInt(), color=request.get("argb").getAsInt();
        int light=request.get("light").getAsInt();
        int luminosity=request.get("luminosity").getAsInt();
        Sprite sprite=sprite(side); Collector out=new Collector(); PoseStack pose=new PoseStack();
        CatnipServices.FLUID_HELPER=new ModFluidHelper<>() {
            public int getColor(Object stack, BlockAndTintGetter world, BlockPos pos) { return color; }
            public int getLuminosity(Object stack) { return luminosity; }
            public TextureAtlasSprite getStillTexture(Object stack) { return sprite; }
            public boolean isLighterThanAir(Object stack) { return false; }
            public Object toStack(FluidState state) { throw new UnsupportedOperationException(); }
        };
        new FluidRenderHelper<>().renderFluidBox(new Object(),b[0],b[1],b[2],b[3],b[4],b[5],out,pose,
            light,request.get("bottom").getAsBoolean(),false);
        return out.vertices;
    }
    public static void main(String[] args) throws Exception {
        try(BufferedReader input=new BufferedReader(new InputStreamReader(System.in))) {
            for(String line;(line=input.readLine())!=null;) System.out.println(JSON.toJson(run(JsonParser.parseString(line).getAsJsonObject())));
        }
    }
}
