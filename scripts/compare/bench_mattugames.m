% 使い方: octave --no-gui --quiet bench_mattugames.m <nucleolus|prenucleolus> <v.txt>
% MatTuGames の nucl / PreNucl を 1 回計算し、bench.R と同じ形の 1 行の CSV (file,n,seconds,x1 x2 ...) を出す。
% 入力はビット順で、MatTuGames の提携の並びと同じ。MATLAB の linprog の代わりに octave/linprog.m (GLPK) を使う。
args = argv();
method = args{1}; file = args{2};
here = fileparts(mfilename('fullpath'));
addpath(getenv('MATTUGAMES_DIR'));
addpath(fullfile(here, 'octave'));
warning('off', 'Octave:shadowed-function');
v = load(file)';
n = round(log2(numel(v) + 1));
tic;
if strcmp(method, 'nucleolus')
  x = nucl(v);
else
  x = PreNucl(v);
end
seconds = toc;
[~, name, ext] = fileparts(file);
printf('%s,%d,%.6f,%s\n', [name ext], n, seconds, strjoin(arrayfun(@(a) sprintf('%.15g', a), x, 'UniformOutput', false), ' '));
