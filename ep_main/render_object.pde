// 描画オブジェクト。メッシュ（頂点+インデックス）とトランスフォーム履歴を GPU バッファで持つ

class RenderObject {
  float[] vertexData;   // 8 float/頂点（pos.xyz, u, 法線.xyz, v）
  int[] indexData;
  GpuBuffer verticesBuffer;
  GpuBuffer indicesBuffer;
  int[] edgeData;       // ワイヤーフレーム用の辺（2 index/辺）
  GpuBuffer edgesBuffer;
  GpuBuffer transformHistory;
  GpuUniform shaderParams;
  GpuRenderBinding wireframeBinding;
  GpuRenderBinding shadowBinding;
  int historySlot;      // historyPool 内の固定スロット
  Material material;
  boolean wireframeOnly = false;   // 常にワイヤで描き、影も落とさない（範囲表示など）
  float roughness = 0.5;
  float metallic = 0;
  PVector baseColor = new PVector(1, 1, 1);   // color.slang のマテリアルでだけ効く

  // CPU 側の現在トランスフォーム。ここを動かせば履歴に記録される
  PVector position = new PVector();
  PVector scale = new PVector(1, 1, 1);
  Quaternion rotation = quatIdentity();
  int spawnFrame = 0;
  int recordedKeys = 0;

  float spawnTime() {
    return spawnFrame * engine.KEYFRAME_INTERVAL;
  }

  void dispose() {
    verticesBuffer.dispose();
    indicesBuffer.dispose();
    edgesBuffer.dispose();
    transformHistory.dispose();
    shaderParams.dispose();
    engine.freeHistorySlot(historySlot);
  }

  // sceneFrame までに番が来たキーフレームへ、現在のトランスフォームを書き込む。
  // 初回は全スロットを現在のトランスフォームで埋める（シェーダーの二分探索は
  // 全スロットを読むので、未来分も常に有効な値にしておく）。生成後・初描画前に
  // トランスフォームを変更してもキー 0 と食い違わないよう、初期化は描画時まで遅らせる
  void recordKeyframes() {
    int due = engine.sceneFrame - spawnFrame + 1;
    // ギャップが容量以上なら、それより古いキーはどうせ復元不可能なので全埋めで済ませる
    if (recordedKeys == 0 || due - recordedKeys >= engine.KEYFRAME_CAPACITY) {
      float[] init = new float[engine.KEYFRAME_CAPACITY * 12];
      float[] t = transformFloats();
      for (int k = 0; k < engine.KEYFRAME_CAPACITY; k++) {
        System.arraycopy(t, 0, init, k * 12, 12);
      }
      transformHistory.write(init);
      recordedKeys = due;
      return;
    }
    while (recordedKeys < due) {
      transformHistory.write(recordedKeys % engine.KEYFRAME_CAPACITY, transformFloats());
      recordedKeys++;
    }
  }

  // ObjectTransform の std430 レイアウト（vec3 は 16 バイト境界）
  float[] transformFloats() {
    return new float[] {
      position.x, position.y, position.z, 0,
      scale.x, scale.y, scale.z, 0,
      rotation.x, rotation.y, rotation.z, rotation.w,
    };
  }
}
