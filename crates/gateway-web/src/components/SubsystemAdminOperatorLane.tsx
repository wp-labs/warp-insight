import styles from "./SubsystemAdminOperatorLane.module.css";
import { SubsystemAdminOperatorIdentity } from "./SubsystemAdminOperatorIdentity";
import { SubsystemAdminOperator } from "./SubsystemAdminOperator";
import { SubsystemAdminOperatorAccess } from "./SubsystemAdminOperatorAccess";
import { SubsystemHttp } from "./SubsystemHttp";

interface SubsystemAdminOperatorLaneProps {
  children?: React.ReactNode;
}

/**
 * 操作者上下文条：说明"是谁、通过什么通道"在派发运维命令。
 * 管理接口全部在 Admin Token 之后，这里把前提显式写出来，避免误以为免鉴权。
 */
export function SubsystemAdminOperatorLane({}: SubsystemAdminOperatorLaneProps) {
  return (
    <div className={styles.container}>
      <SubsystemAdminOperatorIdentity>
        <SubsystemAdminOperator />
      </SubsystemAdminOperatorIdentity>
      <SubsystemAdminOperatorAccess>
        <SubsystemHttp />
      </SubsystemAdminOperatorAccess>
      <span className={styles.note}>
        命令默认以管理员身份派发，未设置 Admin Token 时提交会被拒绝。
      </span>
    </div>
  );
}
