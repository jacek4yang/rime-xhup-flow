export default defineAppConfig({
  pages: [
    "pages/index/index",
    "pages/learn/index",
    "pages/lesson/index",
    "pages/practice/index",
    "pages/practice-setup/index",
    "pages/session/index",
    "pages/summary/index",
    "pages/keyboard/index",
    "pages/key-detail/index",
    "pages/mistakes/index",
    "pages/settings/index",
  ],
  tabBar: {
    color: "#646a73",
    selectedColor: "#2f6fed",
    backgroundColor: "#ffffff",
    borderStyle: "black",
    list: [
      { pagePath: "pages/index/index", text: "今日" },
      { pagePath: "pages/learn/index", text: "学习" },
      { pagePath: "pages/settings/index", text: "我的" },
    ],
  },
  window: {
    backgroundTextStyle: "light",
    navigationBarBackgroundColor: "#ffffff",
    navigationBarTitleText: "小鹤音形训练",
    navigationBarTextStyle: "black",
  },
});
