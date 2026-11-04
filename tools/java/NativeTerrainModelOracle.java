// Development-only: execute the player's unchanged 1.21.1 FaceBakery and JOML normal transform.
import com.google.gson.*;
import net.minecraft.client.renderer.block.model.*;
import net.minecraft.client.renderer.texture.*;
import net.minecraft.client.resources.model.BlockModelRotation;
import net.minecraft.core.Direction;
import net.minecraft.resources.ResourceLocation;
import org.joml.*;
import sun.misc.Unsafe;
import java.io.*;
import java.lang.reflect.*;
import java.util.*;

public final class NativeTerrainModelOracle {
    static final Gson JSON=new Gson();
    static final class Sprite extends TextureAtlasSprite {
        Sprite(SpriteContents contents,int atlas) {
            super(ResourceLocation.fromNamespaceAndPath("minecraft","textures/atlas/blocks.png"),contents,atlas,atlas,0,0);
        }
    }
    static Sprite sprite(int atlas)throws Exception {
        Field f=Unsafe.class.getDeclaredField("theUnsafe");f.setAccessible(true);
        SpriteContents contents=(SpriteContents)((Unsafe)f.get(null)).allocateInstance(SpriteContents.class);
        for(String name:new String[]{"width","height"}) {
            Field field=SpriteContents.class.getDeclaredField(name);field.setAccessible(true);field.setInt(contents,16);
        }
        return new Sprite(contents,atlas);
    }
    static Object run(JsonObject q)throws Exception {
        float[] from=JSON.fromJson(q.get("from"),float[].class),to=JSON.fromJson(q.get("to"),float[].class);
        float[] uv=JSON.fromJson(q.get("uv"),float[].class),matrix=JSON.fromJson(q.get("pose"),float[].class);
        Sprite sprite=sprite(q.get("atlas").getAsInt());
        BlockElementFace face=new BlockElementFace(null,-1,"#sprite",new BlockFaceUV(uv,q.get("rotation").getAsInt()));
        BakedQuad quad=new FaceBakery().bakeQuad(new Vector3f(from),new Vector3f(to),face,sprite,
            Direction.valueOf(q.get("direction").getAsString().toUpperCase(Locale.ROOT)),BlockModelRotation.X0_Y0,null,true);
        Matrix4f pose=new Matrix4f().set(matrix);
        Matrix3f normalPose=new Matrix3f(pose).invert().transpose();
        int[] data=quad.getVertices();List<Object> result=new ArrayList<>();
        for(int i=0;i<4;i++) {
            int at=i*8;
            Vector3f pos=pose.transformPosition(new Vector3f(Float.intBitsToFloat(data[at]),Float.intBitsToFloat(data[at+1]),Float.intBitsToFloat(data[at+2])));
            int packed=data[at+7];
            Vector3f normal=normalPose.transform(new Vector3f((byte)packed/127f,(byte)(packed>>8)/127f,(byte)(packed>>16)/127f)).normalize();
            float u=(Float.intBitsToFloat(data[at+4])-sprite.getU0())/(sprite.getU1()-sprite.getU0());
            float v=(Float.intBitsToFloat(data[at+5])-sprite.getV0())/(sprite.getV1()-sprite.getV0());
            result.add(Map.of("pos",new float[]{pos.x,pos.y,pos.z},"uv",new float[]{u,v},"normal",new float[]{normal.x,normal.y,normal.z}));
        }
        return result;
    }
    public static void main(String[] args)throws Exception {
        try(BufferedReader input=new BufferedReader(new InputStreamReader(System.in))) {
            for(String line;(line=input.readLine())!=null;)System.out.println(JSON.toJson(run(JsonParser.parseString(line).getAsJsonObject())));
        }
    }
}
