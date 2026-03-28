import appIcon from "../../assets/kachakache-icon.svg";

function BrandPill() {
  return (
    <div className="mac-brand-pill">
      <img src={appIcon} alt="" className="mac-brand-icon" />
      <span className="text-[12px] font-semibold tracking-[-0.01em]">KachaKache</span>
    </div>
  );
}

export default BrandPill;
