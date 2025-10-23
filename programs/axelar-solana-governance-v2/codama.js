import { createCodamaConfig } from "gill";

export default createCodamaConfig({
  idl: "../../target/idl/axelar_solana_governance_v2.json",
  clientJs: "programs/axelar-solana-governance-v2/clients/js/src/generated",
});
