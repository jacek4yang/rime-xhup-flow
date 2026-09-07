import { PropsWithChildren } from "react";
import { useLaunch } from "@tarojs/taro";
import "./app.css";

function App({ children }: PropsWithChildren) {
  useLaunch(() => {
    // 本地优先:无登录、无遥测;进度只落本机存储。
  });

  return children;
}

export default App;
