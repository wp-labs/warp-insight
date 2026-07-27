import styles from "./SubsystemAgentInstallCodeList.module.css";
import { SubsystemX86LinuxAgentInstallCode } from "./SubsystemX86LinuxAgentInstallCode";
import { SubsystemX86LinuxInstallCode } from "./SubsystemX86LinuxInstallCode";
import { SubsystemArmLinuxAgentInstallCode } from "./SubsystemArmLinuxAgentInstallCode";
import { SubsystemArmLinuxInstallCode } from "./SubsystemArmLinuxInstallCode";
import type { AgentInstallCode } from "../api";

interface SubsystemAgentInstallCodeListProps {
  installCode?: AgentInstallCode;
  loading?: boolean;
  children?: React.ReactNode;
}

export function SubsystemAgentInstallCodeList({ installCode, loading }: SubsystemAgentInstallCodeListProps) {
  return (
    <div className={styles.container}>
      <SubsystemX86LinuxAgentInstallCode>
        <SubsystemX86LinuxInstallCode command={installCode?.x86LinuxInstallCode} loading={loading} />
      </SubsystemX86LinuxAgentInstallCode>
      <SubsystemArmLinuxAgentInstallCode>
        <SubsystemArmLinuxInstallCode command={installCode?.armLinuxInstallCode} loading={loading} />
      </SubsystemArmLinuxAgentInstallCode>
    </div>
  );
}
