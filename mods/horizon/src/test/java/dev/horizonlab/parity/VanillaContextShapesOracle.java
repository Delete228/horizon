package dev.horizonlab.parity;

import com.google.gson.*;
import java.io.*;
import java.nio.file.*;
import net.minecraft.core.*;
import net.minecraft.world.entity.*;
import net.minecraft.world.entity.decoration.ArmorStand;
import net.minecraft.world.entity.item.ItemEntity;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.level.*;
import net.minecraft.world.level.block.*;
import net.minecraft.world.level.block.state.*;
import net.minecraft.world.phys.*;
import net.minecraft.world.phys.shapes.*;

/** Real BlockState/context dispatch, inherited empty-context collision, edges and ray faces.
 * Explicit fixture geometry; no mod assets or copied mod implementation is included. */
public final class VanillaContextShapesOracle {
    static class QueryBlock extends Block {
        VoxelShape ordinary, interaction;
        java.util.Map<String,VoxelShape> outlines;
        QueryBlock() { super(BlockBehaviour.Properties.of()); }
        @Override protected VoxelShape getShape(BlockState state, BlockGetter world, BlockPos pos, CollisionContext context) {
            return outlines.getOrDefault(kind(context),ordinary);
        }
        @Override protected VoxelShape getInteractionShape(BlockState state, BlockGetter world, BlockPos pos) { return interaction; }
    }
    static class ContextBlock extends QueryBlock {
        java.util.Map<String,VoxelShape> collisions;
        @Override protected VoxelShape getCollisionShape(BlockState state, BlockGetter world, BlockPos pos, CollisionContext context) {
            return collisions.getOrDefault(kind(context),ordinary);
        }
    }
    static String kind(CollisionContext context) {
        if (!(context instanceof EntityCollisionContext ec)) return "none";
        Entity entity=ec.getEntity();
        return entity instanceof Player ? "player" : entity instanceof ItemEntity ? "item"
            : entity instanceof LivingEntity ? "living" : entity != null ? "other" : "none";
    }
    static java.util.Map<String,VoxelShape> variants(JsonObject fixture,String channel) {
        var result=new java.util.HashMap<String,VoxelShape>();
        for(String kind:new String[]{"item","player","living","other"})
            if(fixture.has(channel+"_"+kind))result.put(kind,shape(fixture.get(channel+"_"+kind)));
        return result;
    }
    static class ItemSnapshot extends ItemEntity {
        ItemSnapshot() { super(null, null); }
        @Override public boolean isDescending() { return false; }
    }
    static class LivingSnapshot extends ArmorStand {
        LivingSnapshot() { super(null, null); }
        @Override public boolean isDescending() { return false; }
        @Override public ItemStack getMainHandItem() { return ItemStack.EMPTY; }
    }
    static VoxelShape shape(JsonElement boxes) {
        VoxelShape result=Shapes.empty();
        for (var v:boxes.getAsJsonArray()) { var b=v.getAsJsonArray();result=Shapes.or(result,Shapes.create(new AABB(b.get(0).getAsDouble(),b.get(1).getAsDouble(),b.get(2).getAsDouble(),b.get(3).getAsDouble(),b.get(4).getAsDouble(),b.get(5).getAsDouble()))); }
        return result;
    }
    static Vec3 vector(JsonElement value) { var v=value.getAsJsonArray();return new Vec3(v.get(0).getAsDouble(),v.get(1).getAsDouble(),v.get(2).getAsDouble()); }
    static void values(PrintWriter out,String label,double... values) { out.print(label);for(double v:values)out.printf(" %016x",Double.doubleToLongBits(v));out.println(); }
    static void emit(PrintWriter out,String label,VoxelShape shape) {
        var boxes=shape.toAabbs();out.println(label+" "+boxes.size());for(var b:boxes)values(out,"box",b.minX,b.minY,b.minZ,b.maxX,b.maxY,b.maxZ);
        var edges=new java.util.ArrayList<double[]>();shape.forAllEdges((a,b,c,d,e,f)->edges.add(new double[]{a,b,c,d,e,f}));out.println("edges "+edges.size());for(var edge:edges)values(out,"edge",edge);
    }
    public static void main(String[] args) throws Exception {
        net.minecraft.SharedConstants.tryDetectVersion();net.minecraft.server.Bootstrap.bootStrap();
        var unsafeClass=Class.forName("sun.misc.Unsafe");var field=unsafeClass.getDeclaredField("theUnsafe");field.setAccessible(true);var unsafe=field.get(null);var allocate=unsafeClass.getMethod("allocateInstance",Class.class);
        var hasCollision=BlockBehaviour.class.getDeclaredField("hasCollision");hasCollision.setAccessible(true);
        var owner=StateHolder.class.getDeclaredField("owner");owner.setAccessible(true);var position=Entity.class.getDeclaredField("position");position.setAccessible(true);
        var player=(VanillaCollisionContextOracle.PlayerSnapshot)allocate.invoke(unsafe,VanillaCollisionContextOracle.PlayerSnapshot.class);player.held=ItemStack.EMPTY;
        var item=(ItemSnapshot)allocate.invoke(unsafe,ItemSnapshot.class);var living=(LivingSnapshot)allocate.invoke(unsafe,LivingSnapshot.class);
        var other=(VanillaCollisionContextOracle.MarkerSnapshot)allocate.invoke(unsafe,VanillaCollisionContextOracle.MarkerSnapshot.class);
        for(Entity e:new Entity[]{player,item,living,other})position.set(e,new Vec3(0,2,0));
        var world=(CollisionGetter)java.lang.reflect.Proxy.newProxyInstance(CollisionGetter.class.getClassLoader(),new Class<?>[]{CollisionGetter.class},(proxy,method,values)->{throw new AssertionError("unexpected fixture world query: "+method);});
        Path dir=Path.of(args[1]);
        try(var out=new PrintWriter(Files.newBufferedWriter(dir.resolve(args[0]+".txt")));var lines=Files.lines(dir.resolve("inputs.txt"))) {
            for(String line:(Iterable<String>)lines::iterator) {
                var f=JsonParser.parseString(line).getAsJsonObject();
                var collisions=variants(f,"collision");
                var block=(QueryBlock)allocate.invoke(unsafe,collisions.isEmpty()?QueryBlock.class:ContextBlock.class);
                hasCollision.setBoolean(block,true);
                block.ordinary=shape(f.get("default"));block.outlines=variants(f,"outline");block.interaction=f.has("interaction")?shape(f.get("interaction")):Shapes.empty();
                if(block instanceof ContextBlock cb)cb.collisions=collisions;
                var state=(BlockState)allocate.invoke(unsafe,BlockState.class);owner.set(state,block);
                var context=switch(f.get("kind").getAsString()) {case "player"->CollisionContext.of(player);case "item"->CollisionContext.of(item);case "living"->CollisionContext.of(living);case "other"->CollisionContext.of(other);case "position"->CollisionContext.positionContext(2);default->CollisionContext.empty();};
                var outline=state.getShape(EmptyBlockGetter.INSTANCE,BlockPos.ZERO,context);var collision=context.getCollisionShape(state,world,BlockPos.ZERO);
                emit(out,"outline",outline);emit(out,"collision",collision);
                var b=f.getAsJsonArray("bounds");values(out,"move",collision.collide(Direction.Axis.Y,new AABB(vector(b.get(0)),vector(b.get(1))),f.get("dy").getAsDouble()));
                var hit=EmptyBlockGetter.INSTANCE.clipWithInteractionOverride(vector(f.get("from")),vector(f.get("to")),BlockPos.ZERO,outline,state);
                if(hit==null)out.println("miss");else {out.println("hit "+hit.getDirection().ordinal()+" "+hit.isInside());var p=hit.getLocation();values(out,"point",p.x,p.y,p.z);}
            }
        }
    }
}
