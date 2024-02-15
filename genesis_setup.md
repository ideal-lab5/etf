# A guide on setting up the network with multiple initial authorities

Suppose we want three intial network authorities.

The goal is to construct a shares for the initial network authority set.

1. Generate keys for each authority. This generates the paillier keys for   
``` sh
for i in {1..3}
do
   node etf generate --output keys.$i
   node etf inspect --keys keys.$i --output pk.$i
done
```
2. Generate shares for all keys
