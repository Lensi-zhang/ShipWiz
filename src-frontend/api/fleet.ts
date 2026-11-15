import { command } from "./tauri";
import type { FleetState, NodeInput, ShipNode } from "../types";

export const fleetApi = {
  getState: () => command<FleetState>("get_fleet_state"),
  create: (input: NodeInput) => command<FleetState>("create_node", { input }),
  update: (id: string, input: NodeInput) =>
    command<FleetState>("update_node", { id, input }),
  remove: (id: string) => command<FleetState>("remove_node", { id }),
  setCurrent: (id: string) => command<FleetState>("set_current_node", { id }),
  test: (id: string) => command<ShipNode>("test_node", { id }),
};
