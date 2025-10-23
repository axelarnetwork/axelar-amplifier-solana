import { createCodamaConfig } from "gill";

export default createCodamaConfig({
  idl: "../../target/idl/axelar_solana_gateway_v2.json",
  clientJs: "programs/axelar-solana-gateway-v2/clients/js/src/generated",
});
