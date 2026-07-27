import styles from "./SubsystemAdminOperatorLane.module.css";
import { SubsystemAdminOperatorIdentity } from "./SubsystemAdminOperatorIdentity";
import { SubsystemAdminOperator } from "./SubsystemAdminOperator";
import { SubsystemAdminOperatorAccess } from "./SubsystemAdminOperatorAccess";
import { SubsystemHttp } from "./SubsystemHttp";

interface SubsystemAdminOperatorLaneProps {
  children?: React.ReactNode;
}

export function SubsystemAdminOperatorLane({  }: SubsystemAdminOperatorLaneProps) {
  return (
    <div className={styles.container}>
      <SubsystemAdminOperatorIdentity>
        <SubsystemAdminOperator />
      </SubsystemAdminOperatorIdentity>
      <SubsystemAdminOperatorAccess>
        <SubsystemHttp />
      </SubsystemAdminOperatorAccess>
    </div>
  );
}
