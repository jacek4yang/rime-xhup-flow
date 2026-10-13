import { createElement } from "react";
import type { PropsWithChildren } from "react";
import { useLaunch } from "@tarojs/taro";
import { TrainerDataProvider } from "./lib/trainer-data";
import "./app.css";

function App({ children }: PropsWithChildren) {
  useLaunch(() => {
    // 本地优先:无登录、无遥测;进度只落本机存储。
  });

  return createElement(TrainerDataProvider, null, children);
}

export default App;
