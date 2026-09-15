# gpubridge サンプル

Processing から wgpu の compute shader を使うサンプル集。

| サンプル | 言語 | 見どころ |
|---|---|---|
| `life/` | WGSL | 最小構成。texture ping-pong、`dispatch`/`show`/`submit` の基本形 |
| `boids/` | Slang | `kernelSlang()`、storage buffer の ping-pong、uniform、1 フレーム 3 パス |
| `reaction_diffusion/` | Slang | Gray-Scott。RGBA32F、フレーム内反復（8 ステップ/フレーム）、マウス入力 |
| `more_boids/` | Slang | 空間分割（uniform grid + atomic + 実体コピー）で 1,000,000 体の boids。5 パス構成 |
| `boids_render/` | Slang | boids のシミュレーションはそのままに、描画を compute の splat から render pipeline（vertex/fragment + ADD ブレンド）に置き換え。`GpuRenderer`/`GpuRenderBinding`/`gpu.clear()`、vertex pulling（SV_InstanceID） |
| `suzanne/` | Slang | OBJ メッシュ（スザンヌ）の 3D 描画。`drawIndexed()`・`gpu.buffer(n, stride)`・スムーズ法線・`depthTest(true)`、pde 内の小さな OBJ ローダ、マウスドラッグで回転 |
| `indirect_cull/` | WGSL | GPU カリング + 間接実行。`copyBufferToBuffer()`（引数バッファのリセット）・`dispatchIndirect()`（GPU が決めた個数だけ compute）・`drawIndexedIndirect()`（GPU が決めた個数だけ描画）。マウス周りの粒子だけを描く。「何個描くか」を CPU が一度も知らない |

## 実行

Processing IDE で開くか:

```
processing-java --sketch=examples/life --run
```

各スケッチの `code/` は `ep_main/code` へのシンボリックリンク。
バイナリを更新したら `../build.sh` を実行すれば全サンプルに反映される。

## 新しいサンプルを作るとき

```
mkdir -p examples/mysketch/data
ln -s ../../../code examples/mysketch/code
```

あとは `mysketch/mysketch.pde` と `data/*.wgsl`（または `*.slang`）を書くだけ。
