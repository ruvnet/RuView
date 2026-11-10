import torch
from collections import OrderedDict

# Write a trivial tensor to a pt file
a= torch.tensor([[1,2,3,4], [5,6,7,8]])
o = OrderedDict()
o["test"] = a

# Write a trivial tensor to a pt file
torch.save(o, "test.pt")

############################################################################################################
# Write a trivial tensor to a pt file with a key
torch.save({"model_state_dict": o}, "test_with_key.pt")

############################################################################################################
# Create a tensor with fortran contiguous memory layout
import numpy as np

# Step 1: Create a 3D NumPy array with Fortran order using a range of numbers
# For example, creating a 2x3x4 array
array_fortran = np.asfortranarray(np.arange(1, 2*3*4 + 1).reshape(2, 3, 4))

# Verify the memory order
print("Is Fortran contiguous (F order):", array_fortran.flags['F_CONTIGUOUS'])  # Should be True
print("Is C contiguous (C order):", array_fortran.flags['C_CONTIGUOUS'])  # Should be False

# Step 2: Convert the NumPy array to a PyTorch tensor
tensor_fortran = torch.from_numpy(array_fortran)

# Verify the tensor layout
print("Tensor stride:", tensor_fortran.stride())  # Stride will reflect the Fortran memory layout

# Step 3: Save the PyTorch tensor to a .pth file
torch.save({"tensor_fortran": tensor_fortran}, 'fortran_tensor_3d.pth')

print("3D Tensor saved with Fortran layout.")
