import numpy as np
import sys

m = int(sys.argv[1])
n = int(sys.argv[2])
test_num = int(sys.argv[3])
mat1 = np.random.rand(m * n)
mat2 = np.random.rand(m * n)
np.save("examples/test_{}.npy".format(test_num), mat1)
np.save("examples/test_{}.npy".format(test_num + 1), mat2)