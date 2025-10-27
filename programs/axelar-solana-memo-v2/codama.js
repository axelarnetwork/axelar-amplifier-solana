import { createCodamaConfig } from "gill";

export default createCodamaConfig({
  idl: "../../target/idl/memo.json",
  clientJs: "programs/axelar-solana-memo-v2/clients/js/src/generated",
});
