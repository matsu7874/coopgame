function tf = verLessThan(name, version)
  % MatTuGames は verLessThan('matlab', '24.1.0') で MATLAB の版を見て LP の解法を選ぶ。
  % Octave にはこの問い合わせがないので、古い MATLAB として扱う (解法の指定は linprog.m で使わない)。
  if strcmpi(name, 'matlab')
    tf = true;
  else
    error('verLessThan: only "matlab" is supported');
  end
end
