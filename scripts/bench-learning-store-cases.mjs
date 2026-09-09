export const learningCases = [];
for (const [rows, padding] of [[1000, 1024], [10000, 1024], [50000, 1024], [50000, 4096]]) {
  for (const kind of ["recent", "overview", "record"]) {
    const repetitions = kind === "recent" ? 100 : kind === "overview" ? 30 : 10;
    learningCases.push({ name: `${kind}-${rows}-${padding}`, args: ["learning", kind, rows, padding, repetitions].map(String) });
  }
}

export function learningPairs(round) {
  const offset = round * 5 % learningCases.length;
  const rotated = [...learningCases.slice(offset), ...learningCases.slice(0, offset)];
  return rotated.map(item => {
    const phases = ["before", "after"];
    // Use the original index: the rotating offset must not cancel the round's parity change.
    if ((round + learningCases.indexOf(item)) % 2) phases.reverse();
    return { item, phases };
  });
}

export function learningLoadPairs(round) {
  return learningPairs(round).filter(({ item }) => item.args[1] === "recent");
}
