import { createCodamaConfig } from "gill";

export default createCodamaConfig({
  idl: "../../target/idl/axelar_solana_gas_service_v2.json",
  clientJs: "programs/axelar-solana-gas-service-v2/clients/js/src/generated",
});
