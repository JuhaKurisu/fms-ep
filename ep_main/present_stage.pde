// デモが使うオブジェクトの置き場。作ったものを覚えておき、clear() で全部捨てる
class Stage {
  List<RenderObject> owned = new ArrayList<>();
  List<Float> localRadius = new ArrayList<>();   // owned と同じ添字でメッシュの原点からの最大距離を持つ
  List<Boolean> still = new ArrayList<>();        // owned と同じ添字で、動かないオブジェクトなら true

  RenderObject add(float[] vertexData, int[] indexData, PVector position) {
    return add(vertexData, indexData, position, engine.defaultMaterial);
  }

  RenderObject add(float[] vertexData, int[] indexData, PVector position, Material material) {
    return addObject(vertexData, indexData, position, material, false);
  }

  // 動かないオブジェクト用。動かないものは delay をかけた光でも見た目が現在光と同じで、
  // 待つべき時間が無いのでデモの長さを決める farthestDistance() から外す
  RenderObject addStill(float[] vertexData, int[] indexData, PVector position) {
    return addStill(vertexData, indexData, position, engine.defaultMaterial);
  }

  RenderObject addStill(float[] vertexData, int[] indexData, PVector position, Material material) {
    return addObject(vertexData, indexData, position, material, true);
  }

  RenderObject addObject(float[] vertexData, int[] indexData, PVector position, Material material, boolean isStill) {
    RenderObject o = engine.createObject(vertexData, indexData, position, material);
    owned.add(o);
    localRadius.add(meshRadius(vertexData));
    still.add(isStill);
    return o;
  }

  // 頂点データ（8 float/頂点、先頭 3 つが pos.xyz）から原点までの最大距離を求める
  float meshRadius(float[] vertexData) {
    float r2 = 0;
    for (int i = 0; i < vertexData.length; i += 8) {
      float x = vertexData[i], y = vertexData[i + 1], z = vertexData[i + 2];
      r2 = max(r2, x * x + y * y + z * z);
    }
    return sqrt(r2);
  }

  void clear() {
    for (RenderObject o : owned) o.dispose();
    owned.clear();
    localRadius.clear();
    still.clear();
  }

  void recordKeyframes() {
    for (RenderObject o : owned) o.recordKeyframes();
  }

  void collect(List<RenderObject> objects) {
    for (RenderObject o : owned) objects.add(o);
  }

  // warmupFrames の自動計算用。原点だけでなくメッシュの大きさ（スケール込みの
  // 半径）も加え、長いメッシュの遠端が届き切っていない見た目を避ける
  float farthestDistance(PVector from) {
    return farthestDistance(from, false);
  }

  // 履歴を遡る長さを決めるときは includeStill。動かないものも、生成直後は光がまだ
  // 届いていない扱いで消えるので、待つ時間には数える
  float farthestDistance(PVector from, boolean includeStill) {
    float far = 0;
    for (int i = 0; i < owned.size(); i++) {
      if (!includeStill && still.get(i)) continue;
      RenderObject o = owned.get(i);
      PVector s = o.scale;
      float maxScale = max(abs(s.x), max(abs(s.y), abs(s.z)));
      far = max(far, PVector.dist(from, o.position) + localRadius.get(i) * maxScale);
    }
    return far;
  }
}
