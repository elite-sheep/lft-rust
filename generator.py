import numpy as np
import sys

m = int(sys.argv[1])
n = int(sys.argv[2])
test_num = int(sys.argv[3])
mat = np.random.rand(m, n)
np.save("examples/test_{}.npy".format(test_num), mat)