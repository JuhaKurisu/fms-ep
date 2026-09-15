// OBJ の読み込み。usemtl ごとに分けて、他のメッシュと同じ 8 float/頂点
// (pos.xyz, u, 法線.xyz, v) にする。色は mtllib の Kd から取る

class ObjPart {
  String material;
  float[] vertices;
  int[] indices;
  PVector baseColor = new PVector(1, 1, 1);
}

// 組み立て中のパート。lookup は "v/vt/vn" から頂点番号へ
class ObjBuilder {
  String material;
  FloatList vertices = new FloatList();
  IntList indices = new IntList();
  HashMap<String, Integer> lookup = new HashMap<String, Integer>();

  ObjBuilder(String material) { this.material = material; }

  ObjPart finish(HashMap<String, PVector> colors) {
    ObjPart part = new ObjPart();
    part.material = material;
    part.vertices = vertices.array();
    part.indices = indices.array();
    PVector kd = colors.get(material);
    if (kd != null) part.baseColor = kd.copy();
    return part;
  }
}

// 分割パイプラインは頂点配列の参照でメッシュを同一視するので、同じファイルを
// 使い回せばプールへのコピーが 1 回で済む
HashMap<String, List<ObjPart>> objCache = new HashMap<String, List<ObjPart>>();

List<ObjPart> loadObj(String path) {
  List<ObjPart> cached = objCache.get(path);
  if (cached != null) return cached;

  List<PVector> positions = new ArrayList<PVector>();
  List<PVector> normals = new ArrayList<PVector>();
  List<PVector> uvs = new ArrayList<PVector>();
  HashMap<String, PVector> colors = new HashMap<String, PVector>();

  List<ObjPart> parts = new ArrayList<ObjPart>();
  List<ObjBuilder> builders = new ArrayList<ObjBuilder>();
  ObjBuilder current = new ObjBuilder("");
  builders.add(current);

  for (String line : loadStrings(path)) {
    String[] t = splitTokens(line);
    if (t.length == 0 || t[0].startsWith("#")) continue;
    if (t[0].equals("v") && t.length >= 4) {
      positions.add(new PVector(float(t[1]), float(t[2]), float(t[3])));
    } else if (t[0].equals("vn") && t.length >= 4) {
      normals.add(new PVector(float(t[1]), float(t[2]), float(t[3])));
    } else if (t[0].equals("vt") && t.length >= 3) {
      uvs.add(new PVector(float(t[1]), float(t[2])));
    } else if (t[0].equals("mtllib") && t.length >= 2) {
      colors.putAll(loadMtlColors(siblingPath(path, t[1])));
    } else if (t[0].equals("usemtl") && t.length >= 2) {
      current = builderFor(builders, t[1]);
    } else if (t[0].equals("f") && t.length >= 4) {
      addFace(current, positions, normals, uvs, t);
    }
  }

  for (ObjBuilder b : builders) {
    if (b.indices.size() > 0) parts.add(b.finish(colors));
  }
  objCache.put(path, parts);
  return parts;
}

// 同じマテリアルが飛び飛びに現れても 1 パートにまとめる
ObjBuilder builderFor(List<ObjBuilder> builders, String material) {
  for (ObjBuilder b : builders) {
    if (b.material.equals(material)) return b;
  }
  ObjBuilder b = new ObjBuilder(material);
  builders.add(b);
  return b;
}

// 多角形は扇状に三角形へ。法線が無い面は面法線を使う
void addFace(ObjBuilder b, List<PVector> positions, List<PVector> normals, List<PVector> uvs, String[] t) {
  int corners = t.length - 1;
  int[][] refs = new int[corners][];
  for (int i = 0; i < corners; i++) refs[i] = faceRefs(t[i + 1], positions.size(), uvs.size(), normals.size());

  for (int k = 2; k < corners; k++) {
    int[][] tri = {refs[0], refs[k - 1], refs[k]};
    boolean hasNormals = tri[0][2] >= 0 && tri[1][2] >= 0 && tri[2][2] >= 0;
    PVector faceNormal = hasNormals
      ? null
      : triangleNormal(positions.get(tri[0][0]), positions.get(tri[1][0]), positions.get(tri[2][0]));
    for (int[] r : tri) b.indices.append(vertexIndex(b, positions, normals, uvs, r, faceNormal));
  }
}

// "v/vt/vn" を 0 始まりの添字へ。負の値は末尾からの相対、欠けていれば -1
int[] faceRefs(String token, int positionCount, int uvCount, int normalCount) {
  String[] f = split(token, '/');
  int[] counts = {positionCount, uvCount, normalCount};
  int[] out = {-1, -1, -1};
  for (int i = 0; i < 3 && i < f.length; i++) {
    if (f[i].length() == 0) continue;
    int index = int(f[i]);
    out[i] = index > 0 ? index - 1 : counts[i] + index;
  }
  return out;
}

PVector triangleNormal(PVector a, PVector b, PVector c) {
  PVector n = PVector.sub(b, a).cross(PVector.sub(c, a));
  return n.magSq() > 1e-20 ? n.normalize() : new PVector(0, 1, 0);
}

// 同じ組み合わせの頂点は共有する。面法線を使う面は面ごとに別の頂点にする
int vertexIndex(ObjBuilder b, List<PVector> positions, List<PVector> normals, List<PVector> uvs, int[] r, PVector faceNormal) {
  String key = faceNormal == null ? r[0] + "/" + r[1] + "/" + r[2] : null;
  if (key != null) {
    Integer found = b.lookup.get(key);
    if (found != null) return found;
  }

  PVector p = positions.get(r[0]);
  PVector n = faceNormal != null ? faceNormal : normals.get(r[2]);
  PVector uv = r[1] >= 0 ? uvs.get(r[1]) : new PVector();
  int index = b.vertices.size() / 8;
  b.vertices.append(new float[] {p.x, p.y, p.z, uv.x, n.x, n.y, n.z, uv.y});
  if (key != null) b.lookup.put(key, index);
  return index;
}

HashMap<String, PVector> loadMtlColors(String path) {
  HashMap<String, PVector> colors = new HashMap<String, PVector>();
  String[] lines = loadStrings(path);
  if (lines == null) return colors;
  String current = "";
  for (String line : lines) {
    String[] t = splitTokens(line);
    if (t.length == 0 || t[0].startsWith("#")) continue;
    if (t[0].equals("newmtl") && t.length >= 2) current = t[1];
    else if (t[0].equals("Kd") && t.length >= 4) colors.put(current, new PVector(float(t[1]), float(t[2]), float(t[3])));
  }
  return colors;
}

String siblingPath(String path, String name) {
  int slash = path.lastIndexOf('/');
  return slash < 0 ? name : path.substring(0, slash + 1) + name;
}

// パート全体の境界箱。原点合わせや大きさの正規化に使う
PVector objSize(List<ObjPart> parts) {
  PVector lo = new PVector(Float.MAX_VALUE, Float.MAX_VALUE, Float.MAX_VALUE);
  PVector hi = new PVector(-Float.MAX_VALUE, -Float.MAX_VALUE, -Float.MAX_VALUE);
  for (ObjPart part : parts) {
    for (int i = 0; i < part.vertices.length; i += 8) {
      lo.set(min(lo.x, part.vertices[i]), min(lo.y, part.vertices[i + 1]), min(lo.z, part.vertices[i + 2]));
      hi.set(max(hi.x, part.vertices[i]), max(hi.y, part.vertices[i + 1]), max(hi.z, part.vertices[i + 2]));
    }
  }
  return PVector.sub(hi, lo);
}
