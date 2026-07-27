import styles from "./SubsystemAgentRuntimeStatusResult.module.css";
import { SubsystemAgentInstanceText } from "./SubsystemAgentInstanceText";
import { SubsystemInstanceId } from "./SubsystemInstanceId";
import { SubsystemAgentVersionText } from "./SubsystemAgentVersionText";
import { SubsystemVersion } from "./SubsystemVersion";
import { SubsystemAgentOnlineStatusBadge } from "./SubsystemAgentOnlineStatusBadge";
import { SubsystemStatus } from "./SubsystemStatus";
import { SubsystemAgentHealthBadge } from "./SubsystemAgentHealthBadge";
import { SubsystemHealth } from "./SubsystemHealth";
import { SubsystemAgentLastSeenAtText } from "./SubsystemAgentLastSeenAtText";
import { SubsystemLastSeenAt } from "./SubsystemLastSeenAt";

interface SubsystemAgentRuntimeStatusResultProps {
  children?: React.ReactNode;
}

export function SubsystemAgentRuntimeStatusResult({  }: SubsystemAgentRuntimeStatusResultProps) {
  return (
    <div className={styles.container}>
      <SubsystemAgentInstanceText>
        <SubsystemInstanceId />
      </SubsystemAgentInstanceText>
      <SubsystemAgentVersionText>
        <SubsystemVersion />
      </SubsystemAgentVersionText>
      <SubsystemAgentOnlineStatusBadge>
        <SubsystemStatus />
      </SubsystemAgentOnlineStatusBadge>
      <SubsystemAgentHealthBadge>
        <SubsystemHealth />
      </SubsystemAgentHealthBadge>
      <SubsystemAgentLastSeenAtText>
        <SubsystemLastSeenAt />
      </SubsystemAgentLastSeenAtText>
    </div>
  );
}
