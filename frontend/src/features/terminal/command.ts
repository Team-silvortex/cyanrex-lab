export type CommandType =
  | "ListModules"
  | "StartModule"
  | "StopModule"
  | "RunExperiment";

export type ModuleInfo = {
  name: string;
  status: string;
};

export type CommandRequest = {
  commandType: CommandType;
  moduleName?: string;
};

export type CommandResponse = {
  ok: boolean;
  commandType: CommandType;
  message: string;
  modules?: ModuleInfo[];
  module?: ModuleInfo;
  nextPath?: string;
};

export const commandNeedsModuleName = (commandType: CommandType): boolean =>
  commandType === "StartModule" || commandType === "StopModule";

export function buildCommandRequest(
  commandType: CommandType,
  moduleName: string,
): CommandRequest {
  if (!commandNeedsModuleName(commandType)) {
    return { commandType };
  }

  const normalizedName = moduleName.trim();
  if (!normalizedName) {
    throw new Error("module name is required");
  }
  return { commandType, moduleName: normalizedName };
}

export function summarizeCommandResponse(response: CommandResponse): string {
  if (response.module) {
    return `${response.module.name}: ${response.module.status}`;
  }
  if (response.modules && response.modules.length > 0) {
    return response.modules
      .map((module) => `${module.name}: ${module.status}`)
      .join(", ");
  }
  return response.message;
}
