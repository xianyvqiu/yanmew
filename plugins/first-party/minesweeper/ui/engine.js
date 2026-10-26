/* 棋盘规则无DOM、无钱包副作用，供界面及定向测试共同使用。 */
window.MinesweeperRules = (() => {
  const levels = { easy: { rows: 8, columns: 8, mines: 10, cost: 5, label: '初级' }, medium: { rows: 16, columns: 16, mines: 40, cost: 20, label: '中级' }, hard: { rows: 16, columns: 30, mines: 99, cost: 50, label: '高级' } };
  function neighbors(board, index) {
    const row = Math.floor(index / board.columns), column = index % board.columns, result = [];
    for (let y = -1; y <= 1; y++) for (let x = -1; x <= 1; x++) {
      if ((!x && !y) || row + y < 0 || row + y >= board.rows || column + x < 0 || column + x >= board.columns) continue;
      result.push((row + y) * board.columns + column + x);
    }
    return result;
  }
  function create(level) {
    if (!levels[level]) throw new Error('请选择有效难度');
    return { ...levels[level], level, status: 'ready', moves: 0, cells: Array.from({ length: levels[level].rows * levels[level].columns }, () => ({ mine: false, number: 0, open: false, mark: 0 })) };
  }
  function seed(board, safe, random) {
    const excluded = new Set([safe, ...neighbors(board, safe)]);
    const positions = board.cells.map((_, index) => index).filter(index => !excluded.has(index));
    for (let index = positions.length - 1; index > 0; index--) {
      const other = Math.floor(random() * (index + 1)); [positions[index], positions[other]] = [positions[other], positions[index]];
    }
    positions.slice(0, board.mines).forEach(index => { board.cells[index].mine = true; });
    board.cells.forEach((cell, index) => { cell.number = neighbors(board, index).filter(next => board.cells[next].mine).length; });
    board.status = 'playing';
  }
  function reveal(board, positions) {
    const queue = [...positions];
    while (queue.length) {
      const index = queue.pop(), cell = board.cells[index];
      if (!cell || cell.open || cell.mark === 1) continue;
      cell.open = true; cell.mark = 0;
      if (cell.mine) { board.status = 'lost'; return; }
      if (!cell.number) queue.push(...neighbors(board, index).filter(next => !board.cells[next].open));
    }
    if (board.cells.every(cell => cell.mine || cell.open)) {
      board.status = 'won'; board.cells.forEach(cell => { if (cell.mine) cell.mark = 1; });
    }
  }
  function open(original, index, random = Math.random) {
    const board = structuredClone(original), cell = board.cells[index];
    if (!cell || cell.open || cell.mark === 1 || ['won', 'lost'].includes(board.status)) return board;
    if (board.status === 'ready') seed(board, index, random);
    board.moves++; reveal(board, [index]); return board;
  }
  function mark(original, index) {
    const board = structuredClone(original), cell = board.cells[index];
    if (cell && !cell.open && !['won', 'lost'].includes(board.status)) cell.mark = (cell.mark + 1) % 3;
    return board;
  }
  function chord(original, index) {
    const board = structuredClone(original), cell = board.cells[index];
    if (board.status !== 'playing' || !cell?.open || !cell.number) return board;
    const adjacent = neighbors(board, index);
    if (adjacent.filter(next => board.cells[next].mark === 1).length === cell.number) { board.moves++; reveal(board, adjacent); }
    return board;
  }
  return { levels, create, open, mark, chord, neighbors };
})();
