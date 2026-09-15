// data/luts/*.cube をファイル名順に並べ、選ばれた 1 つをバッファに載せる。並びと補間は reference/src/lut.rs と同じ
class Tonemap {
  String[] names = new String[0];
  File[] files = new File[0];
  GpuBuffer buffer;
  int size = 0;
  int loaded = -1;

  Tonemap(GpuWindow gpu) {
    File dir = dataFile("luts");
    File[] found = dir.isDirectory() ? dir.listFiles((d, n) -> n.endsWith(".cube")) : null;
    if (found != null) {
      java.util.Arrays.sort(found);
      files = found;
      names = new String[found.length];
      for (int i = 0; i < found.length; i++) names[i] = found[i].getName();
    }
    buffer = gpu.buffer(max(largestSize(), 1), 4);
  }

  int largestSize() {
    int m = 0;
    for (File f : files) {
      int n = readSize(f);
      m = max(m, n * n * n * 3);
    }
    return m;
  }

  int readSize(File f) {
    for (String line : loadStrings(f.getPath())) {
      if (line.trim().startsWith("LUT_3D_SIZE")) return int(splitTokens(line)[1]);
    }
    return 0;
  }

  void select(int index) {
    if (files.length == 0) { size = 0; return; }
    index = constrain(index, 0, files.length - 1);
    if (index == loaded) return;
    int n = 0;
    ArrayList<Float> values = new ArrayList<Float>();
    for (String raw : loadStrings(files[index].getPath())) {
      String line = raw.trim();
      if (line.isEmpty() || line.startsWith("#") || line.startsWith("TITLE") || line.startsWith("DOMAIN_")) continue;
      if (line.startsWith("LUT_3D_SIZE")) { n = int(splitTokens(line)[1]); continue; }
      for (String t : splitTokens(line)) values.add(Float.parseFloat(t));
    }
    if (n < 2 || values.size() != n * n * n * 3) {
      println("tonemap: " + files[index].getName() + " の形式が違います");
      size = 0;
      loaded = index;
      return;
    }
    float[] data = new float[values.size()];
    for (int i = 0; i < data.length; i++) data[i] = values.get(i);
    buffer.write(data);
    size = n;
    loaded = index;
    println("tonemap: " + files[index].getName() + " (" + n + "^3)");
  }
}
