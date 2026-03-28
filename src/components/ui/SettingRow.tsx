import type { ReactNode } from "react";

type SettingRowProps = {
  label: string;
  description?: string;
  control: ReactNode;
  statusChip?: ReactNode;
  variant?: "glass" | "native";
};

function SettingRow({ label, description, control, statusChip }: SettingRowProps) {
  return (
    <div className="mac-setting-row">
      <div className="min-w-0">
        <p className="mac-setting-label">{label}</p>
        {description && <p className="mac-setting-description">{description}</p>}
      </div>
      <div className="flex items-center gap-1.5 text-[12px]">
        {statusChip}
        {control}
      </div>
    </div>
  );
}

export default SettingRow;
