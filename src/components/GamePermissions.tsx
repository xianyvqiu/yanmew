import type { GamePermissionChoice } from "../types";

export function permissionLabel(choice: GamePermissionChoice) {
  return choice.permission === "wallet.modify" ? "修改全局喵币（可增加或扣除）" : `联网访问 ${choice.scope}`;
}
export default function GamePermissions({ choices, disabled, onChange }: {
  choices: GamePermissionChoice[]; disabled: boolean;
  onChange: (choices: GamePermissionChoice[]) => void;
}) {
  return <div className="ym-game-permissions">{choices.length === 0 ? <p className="ym-page-note">此插件未申请钱包或联网权限。</p> : choices.map((choice, index) =>
    <label className="ym-field" key={`${choice.permission}:${choice.scope}`}>{permissionLabel(choice)}
      <select aria-label={permissionLabel(choice)} value={choice.decision} disabled={disabled} onChange={event => onChange(choices.map((item, at) => at === index ? { ...item, decision: event.target.value as GamePermissionChoice["decision"] } : item))}>
        <option value="pending">待确认</option><option value="allowed">允许</option><option value="denied">拒绝</option>
      </select>
    </label>)}</div>;
}
