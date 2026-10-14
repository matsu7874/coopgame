function [x, fval, exitflag, output, lambda] = linprog(f, A, b, Aeq, beq, lb, ub, varargin)
  % MATLAB の linprog と同じ呼び出し方で、Octave の glpk を使って LP を解く互換関数。
  % MatTuGames の nucl.m と PreNucl.m を書き換えずに Octave で動かすために使う。
  % 解法は GLPK の双対単体法。options (MaxIter, 許容誤差など) は使わない。
  % lambda.ineqlin は MATLAB と同じ符号 (最小化で、効いている <= 制約が非負) にする。
  if nargin < 4, Aeq = []; end
  if nargin < 5, beq = []; end
  if nargin < 6, lb = []; end
  if nargin < 7, ub = []; end
  nx = numel(f);
  if isempty(lb), lb = -Inf(nx, 1); end
  if isempty(ub), ub = Inf(nx, 1); end
  M = [A; Aeq];
  rhs = [b(:); beq(:)];
  ctype = [repmat('U', 1, size(A, 1)), repmat('S', 1, size(Aeq, 1))];
  param.msglev = 0;
  param.lpsolver = 1;
  param.dual = 2;
  [x, fval, errnum, extra] = glpk(f(:), M, rhs, lb(:), ub(:), ctype, repmat('C', 1, nx), 1, param);
  status = extra.status;
  exitflag = double(errnum == 0 && status == 5);
  output = struct('glpk_status', status);
  lambda.ineqlin = -extra.lambda(1:size(A, 1));
  lambda.eqlin = -extra.lambda(size(A, 1) + 1:end);
end
