import numpy as np
x = np.arange(10)

# Write a npy file.
np.save("test.npy", x)

# Write multiple values to a npz file.
values = { "x": x, "x_plus_one": x + 1 }
np.savez("test.npz", **values)
