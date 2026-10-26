window.Twenty48Rules = (() => {
  function shift(board, direction) {
    if (!['left', 'right', 'up', 'down'].includes(direction)) throw new Error('移动方向无效');
    const next = Array(16).fill(0), routes = []; let score = 0;
    for (let line = 0; line < 4; line++) {
      const positions = Array.from({ length: 4 }, (_, step) => direction === 'left' ? line * 4 + step : direction === 'right' ? line * 4 + 3 - step : direction === 'up' ? step * 4 + line : (3 - step) * 4 + line);
      const occupied = positions.filter(index => board[index]), values = occupied.map(index => board[index]), merged = [];
      for (let index = 0; index < values.length; index++) {
        const destination = positions[merged.length];
        routes.push({ from: occupied[index], to: destination, value: values[index] });
        if (values[index] === values[index + 1]) { const value = values[index] * 2; merged.push(value); score += value; routes.push({ from: occupied[index + 1], to: destination, value: values[index + 1] }); index++; }
        else merged.push(values[index]);
      }
      merged.forEach((value, index) => { next[positions[index]] = value; });
    }
    return { board: next, score, routes, changed: next.some((value, index) => value !== board[index]) };
  }
  function add(board, random = Math.random) {
    const empty = board.map((value, index) => value ? -1 : index).filter(index => index >= 0), next = [...board];
    if (empty.length) next[empty[Math.floor(random() * empty.length)]] = random() < .9 ? 2 : 4;
    return next;
  }
  const canMove = board => board.some(value => value === 0) || ['left', 'up'].some(direction => shift(board, direction).changed);
  const reward = highest => highest >= 2048 ? 200 : highest >= 1024 ? 50 : highest >= 512 ? 25 : highest >= 256 ? 10 : 0;
  return { shift, add, canMove, reward, create: random => add(add(Array(16).fill(0), random), random) };
})();
