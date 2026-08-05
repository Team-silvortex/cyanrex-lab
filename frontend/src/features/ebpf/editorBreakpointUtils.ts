export const parseStoredBreakpoints = (raw: unknown): number[] => {
  if (!Array.isArray(raw)) {
    return [];
  }
  const normalized = raw
    .map((value) => Number(value))
    .filter((value) => Number.isInteger(value) && value >= 1);
  return [...new Set(normalized)].sort((a, b) => a - b);
};

export const clampBreakpoints = (breakpoints: number[], lineCount: number): number[] => {
  if (lineCount <= 0) {
    return [];
  }

  return [...new Set(
    breakpoints.filter((line) => Number.isInteger(line) && line >= 1 && line <= lineCount),
  )].sort((a, b) => a - b);
};

export const sameNumberList = (left: number[], right: number[]): boolean => {
  if (left.length !== right.length) {
    return false;
  }

  for (let index = 0; index < left.length; index += 1) {
    if (left[index] !== right[index]) {
      return false;
    }
  }

  return true;
};
