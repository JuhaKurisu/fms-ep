import imgui.flag.ImGuiCol;
import imgui.type.ImBoolean;
import imgui.type.ImFloat;
import imgui.type.ImInt;

// フィールド初期化順(メインタブが先)に依存しないよう、レジストリとJSONは遅延初期化
ArrayList<PrefsParam> prefsParams;
JSONObject prefsJson;
boolean prefsJsonLoaded;

ArrayList<PrefsParam> prefsRegistry() {
  if (prefsParams == null) prefsParams = new ArrayList<PrefsParam>();
  return prefsParams;
}

void prefsEnsureLoaded() {
  if (prefsJsonLoaded) return;
  prefsJsonLoaded = true;
  File file = dataFile("prefs.json");
  prefsJson = file.isFile() ? loadJSONObject(file) : new JSONObject();
  for (PrefsParam p : prefsRegistry()) p.readJson(prefsJson);
}

void prefsLoad() {
  prefsJsonLoaded = false;
  prefsEnsureLoaded();
}

void prefsSave() {
  prefsEnsureLoaded();
  for (PrefsParam p : prefsRegistry()) {
    p.putJson(prefsJson);
    p.markSaved();
  }
  saveJSONObject(prefsJson, "data/prefs.json");
}

// 未保存の変更も含めた現在値。保存済みの印は変えない
JSONObject prefsCurrentJson() {
  prefsEnsureLoaded();
  JSONObject json = new JSONObject();
  for (PrefsParam p : prefsRegistry()) p.putJson(json);
  return json;
}

float prefsLabelWidthCache;

float prefsLabelWidth() {
  if (prefsLabelWidthCache == 0) {
    float max = 0;
    for (PrefsParam p : prefsRegistry()) {
      max = max(max, ImGui.calcTextSize(p.key).x);
    }
    prefsLabelWidthCache = max + ImGui.getStyle().getItemSpacingX() * 2;
  }
  return prefsLabelWidthCache;
}

abstract class PrefsParam {
  final String key;
  final String hiddenLabel;

  PrefsParam(String key) {
    this.key = key;
    hiddenLabel = "##" + key;
    prefsRegistry().add(this);
    prefsLabelWidthCache = 0;
  }

  void applySaved() {
    if (prefsJsonLoaded) readJson(prefsJson);
  }

  void beginGui(boolean modified) {
    prefsEnsureLoaded();
    if (modified) ImGui.pushStyleColor(ImGuiCol.Text, 1.0, 0.85, 0.4, 1.0);
    ImGui.alignTextToFramePadding();
    ImGui.text(key);
    ImGui.sameLine(prefsLabelWidth());
    ImGui.setNextItemWidth(-1);
  }

  void endGui(boolean modified) {
    if (modified) ImGui.popStyleColor();
  }

  abstract void readJson(JSONObject json);
  abstract void putJson(JSONObject json);
  abstract void markSaved();
}

class PrefsFloat extends PrefsParam {
  float value, def, savedValue;
  float[] buf = new float[1];
  ImFloat fieldBuf = new ImFloat();

  PrefsFloat(String key, float def) {
    super(key);
    this.def = def;
    value = def;
    savedValue = def;
    applySaved();
  }

  float get() {
    prefsEnsureLoaded();
    return value;
  }

  void set(float v) {
    prefsEnsureLoaded();
    value = v;
  }

  void readJson(JSONObject json) {
    if (json.hasKey(key)) value = json.getFloat(key);
    savedValue = value;
  }

  void putJson(JSONObject json) {
    json.setFloat(key, value);
  }

  void markSaved() {
    savedValue = value;
  }

  boolean slider(float min, float max) {
    boolean modified = value != savedValue;
    beginGui(modified);
    buf[0] = value;
    boolean changed = ImGui.sliderFloat(hiddenLabel, buf, min, max);
    if (changed) value = buf[0];
    endGui(modified);
    return changed;
  }

  boolean drag(float speed) {
    boolean modified = value != savedValue;
    beginGui(modified);
    buf[0] = value;
    boolean changed = ImGui.dragFloat(hiddenLabel, buf, speed);
    if (changed) value = buf[0];
    endGui(modified);
    return changed;
  }

  boolean field() {
    boolean modified = value != savedValue;
    beginGui(modified);
    fieldBuf.set(value);
    boolean changed = ImGui.inputFloat(hiddenLabel, fieldBuf);
    if (changed) value = fieldBuf.get();
    endGui(modified);
    return changed;
  }
}

class PrefsInt extends PrefsParam {
  int value, def, savedValue;
  int[] buf = new int[1];
  ImInt fieldBuf = new ImInt();

  PrefsInt(String key, int def) {
    super(key);
    this.def = def;
    value = def;
    savedValue = def;
    applySaved();
  }

  int get() {
    prefsEnsureLoaded();
    return value;
  }

  void set(int v) {
    prefsEnsureLoaded();
    value = v;
  }

  void readJson(JSONObject json) {
    if (json.hasKey(key)) value = json.getInt(key);
    savedValue = value;
  }

  void putJson(JSONObject json) {
    json.setInt(key, value);
  }

  void markSaved() {
    savedValue = value;
  }

  boolean slider(int min, int max) {
    boolean modified = value != savedValue;
    beginGui(modified);
    buf[0] = value;
    boolean changed = ImGui.sliderInt(hiddenLabel, buf, min, max);
    if (changed) value = buf[0];
    endGui(modified);
    return changed;
  }

  boolean drag(float speed) {
    boolean modified = value != savedValue;
    beginGui(modified);
    buf[0] = value;
    boolean changed = ImGui.dragInt(hiddenLabel, buf, speed);
    if (changed) value = buf[0];
    endGui(modified);
    return changed;
  }

  boolean field() {
    boolean modified = value != savedValue;
    beginGui(modified);
    fieldBuf.set(value);
    boolean changed = ImGui.inputInt(hiddenLabel, fieldBuf);
    if (changed) value = fieldBuf.get();
    endGui(modified);
    return changed;
  }

  boolean combo(String[] items) {
    boolean modified = value != savedValue;
    beginGui(modified);
    fieldBuf.set(constrain(value, 0, max(items.length - 1, 0)));
    boolean changed = ImGui.combo(hiddenLabel, fieldBuf, items);
    if (changed) value = fieldBuf.get();
    endGui(modified);
    return changed;
  }
}

class PrefsBool extends PrefsParam {
  boolean value, def, savedValue;
  ImBoolean buf = new ImBoolean();

  PrefsBool(String key, boolean def) {
    super(key);
    this.def = def;
    value = def;
    savedValue = def;
    applySaved();
  }

  boolean get() {
    prefsEnsureLoaded();
    return value;
  }

  void set(boolean v) {
    prefsEnsureLoaded();
    value = v;
  }

  void readJson(JSONObject json) {
    if (json.hasKey(key)) value = json.getBoolean(key);
    savedValue = value;
  }

  void putJson(JSONObject json) {
    json.setBoolean(key, value);
  }

  void markSaved() {
    savedValue = value;
  }

  boolean checkbox() {
    boolean modified = value != savedValue;
    beginGui(modified);
    buf.set(value);
    boolean changed = ImGui.checkbox(hiddenLabel, buf);
    if (changed) value = buf.get();
    endGui(modified);
    return changed;
  }
}

class PrefsVec3 extends PrefsParam {
  PVector value, def, savedValue;
  float[] buf = new float[3];

  PrefsVec3(String key, PVector def) {
    super(key);
    this.def = def.copy();
    value = def.copy();
    savedValue = def.copy();
    applySaved();
  }

  PVector get() {
    prefsEnsureLoaded();
    return value;
  }

  void set(PVector v) {
    prefsEnsureLoaded();
    value.set(v);
  }

  void readJson(JSONObject json) {
    if (json.hasKey(key)) {
      JSONArray a = json.getJSONArray(key);
      value.set(a.getFloat(0), a.getFloat(1), a.getFloat(2));
    }
    savedValue.set(value);
  }

  void putJson(JSONObject json) {
    JSONArray a = new JSONArray();
    a.append(value.x);
    a.append(value.y);
    a.append(value.z);
    json.setJSONArray(key, a);
  }

  void markSaved() {
    savedValue.set(value);
  }

  boolean drag(float speed) {
    boolean modified = !value.equals(savedValue);
    beginGui(modified);
    buf[0] = value.x;
    buf[1] = value.y;
    buf[2] = value.z;
    boolean changed = ImGui.dragFloat3(hiddenLabel, buf, speed);
    if (changed) value.set(buf[0], buf[1], buf[2]);
    endGui(modified);
    return changed;
  }

  boolean field() {
    boolean modified = !value.equals(savedValue);
    beginGui(modified);
    buf[0] = value.x;
    buf[1] = value.y;
    buf[2] = value.z;
    boolean changed = ImGui.inputFloat3(hiddenLabel, buf);
    if (changed) value.set(buf[0], buf[1], buf[2]);
    endGui(modified);
    return changed;
  }
}
