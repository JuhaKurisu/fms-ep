package imguibridge;

import imgui.flag.ImGuiKey;
import java.awt.event.KeyEvent;

/** AWT の keyCode(VK_*)→ ImGuiKey の対応表。 */
final class AwtKeyMap {

  private AwtKeyMap() {}

  // (vk, imguiKey) のペア列。ImGui がナビゲーションやショートカットに使うキーを網羅する。
  private static final int[][] PAIRS = {
    {KeyEvent.VK_TAB, ImGuiKey.Tab},
    {KeyEvent.VK_LEFT, ImGuiKey.LeftArrow},
    {KeyEvent.VK_RIGHT, ImGuiKey.RightArrow},
    {KeyEvent.VK_UP, ImGuiKey.UpArrow},
    {KeyEvent.VK_DOWN, ImGuiKey.DownArrow},
    {KeyEvent.VK_PAGE_UP, ImGuiKey.PageUp},
    {KeyEvent.VK_PAGE_DOWN, ImGuiKey.PageDown},
    {KeyEvent.VK_HOME, ImGuiKey.Home},
    {KeyEvent.VK_END, ImGuiKey.End},
    {KeyEvent.VK_INSERT, ImGuiKey.Insert},
    {KeyEvent.VK_DELETE, ImGuiKey.Delete},
    {KeyEvent.VK_BACK_SPACE, ImGuiKey.Backspace},
    {KeyEvent.VK_SPACE, ImGuiKey.Space},
    {KeyEvent.VK_ENTER, ImGuiKey.Enter},
    {KeyEvent.VK_ESCAPE, ImGuiKey.Escape},
    {KeyEvent.VK_QUOTE, ImGuiKey.Apostrophe},
    {KeyEvent.VK_COMMA, ImGuiKey.Comma},
    {KeyEvent.VK_MINUS, ImGuiKey.Minus},
    {KeyEvent.VK_PERIOD, ImGuiKey.Period},
    {KeyEvent.VK_SLASH, ImGuiKey.Slash},
    {KeyEvent.VK_SEMICOLON, ImGuiKey.Semicolon},
    {KeyEvent.VK_EQUALS, ImGuiKey.Equal},
    {KeyEvent.VK_OPEN_BRACKET, ImGuiKey.LeftBracket},
    {KeyEvent.VK_BACK_SLASH, ImGuiKey.Backslash},
    {KeyEvent.VK_CLOSE_BRACKET, ImGuiKey.RightBracket},
    {KeyEvent.VK_BACK_QUOTE, ImGuiKey.GraveAccent},
    {KeyEvent.VK_CAPS_LOCK, ImGuiKey.CapsLock},
    {KeyEvent.VK_0, ImGuiKey._0},
    {KeyEvent.VK_1, ImGuiKey._1},
    {KeyEvent.VK_2, ImGuiKey._2},
    {KeyEvent.VK_3, ImGuiKey._3},
    {KeyEvent.VK_4, ImGuiKey._4},
    {KeyEvent.VK_5, ImGuiKey._5},
    {KeyEvent.VK_6, ImGuiKey._6},
    {KeyEvent.VK_7, ImGuiKey._7},
    {KeyEvent.VK_8, ImGuiKey._8},
    {KeyEvent.VK_9, ImGuiKey._9},
    {KeyEvent.VK_A, ImGuiKey.A},
    {KeyEvent.VK_B, ImGuiKey.B},
    {KeyEvent.VK_C, ImGuiKey.C},
    {KeyEvent.VK_D, ImGuiKey.D},
    {KeyEvent.VK_E, ImGuiKey.E},
    {KeyEvent.VK_F, ImGuiKey.F},
    {KeyEvent.VK_G, ImGuiKey.G},
    {KeyEvent.VK_H, ImGuiKey.H},
    {KeyEvent.VK_I, ImGuiKey.I},
    {KeyEvent.VK_J, ImGuiKey.J},
    {KeyEvent.VK_K, ImGuiKey.K},
    {KeyEvent.VK_L, ImGuiKey.L},
    {KeyEvent.VK_M, ImGuiKey.M},
    {KeyEvent.VK_N, ImGuiKey.N},
    {KeyEvent.VK_O, ImGuiKey.O},
    {KeyEvent.VK_P, ImGuiKey.P},
    {KeyEvent.VK_Q, ImGuiKey.Q},
    {KeyEvent.VK_R, ImGuiKey.R},
    {KeyEvent.VK_S, ImGuiKey.S},
    {KeyEvent.VK_T, ImGuiKey.T},
    {KeyEvent.VK_U, ImGuiKey.U},
    {KeyEvent.VK_V, ImGuiKey.V},
    {KeyEvent.VK_W, ImGuiKey.W},
    {KeyEvent.VK_X, ImGuiKey.X},
    {KeyEvent.VK_Y, ImGuiKey.Y},
    {KeyEvent.VK_Z, ImGuiKey.Z},
    {KeyEvent.VK_F1, ImGuiKey.F1},
    {KeyEvent.VK_F2, ImGuiKey.F2},
    {KeyEvent.VK_F3, ImGuiKey.F3},
    {KeyEvent.VK_F4, ImGuiKey.F4},
    {KeyEvent.VK_F5, ImGuiKey.F5},
    {KeyEvent.VK_F6, ImGuiKey.F6},
    {KeyEvent.VK_F7, ImGuiKey.F7},
    {KeyEvent.VK_F8, ImGuiKey.F8},
    {KeyEvent.VK_F9, ImGuiKey.F9},
    {KeyEvent.VK_F10, ImGuiKey.F10},
    {KeyEvent.VK_F11, ImGuiKey.F11},
    {KeyEvent.VK_F12, ImGuiKey.F12},
    {KeyEvent.VK_SHIFT, ImGuiKey.LeftShift},
    {KeyEvent.VK_CONTROL, ImGuiKey.LeftCtrl},
    {KeyEvent.VK_ALT, ImGuiKey.LeftAlt},
    {KeyEvent.VK_META, ImGuiKey.LeftSuper},
  };

  /** 対応表にある AWT keyCode の一覧。 */
  static int[] awtCodes() {
    int[] codes = new int[PAIRS.length];
    for (int i = 0; i < PAIRS.length; i++) {
      codes[i] = PAIRS[i][0];
    }
    return codes;
  }

  /** AWT keyCode → ImGuiKey。未対応なら -1。 */
  static int imguiKey(int awtKeyCode) {
    for (int[] p : PAIRS) {
      if (p[0] == awtKeyCode) {
        return p[1];
      }
    }
    return -1;
  }
}
