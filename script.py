from web3 import Web3
w3 = Web3(Web3.HTTPProvider("http://127.0.0.1:8545", request_kwargs={'timeout': 60}))
result = w3.provider.make_request("portal_historyRecursiveFindNodes", ["0xf8ca560dfcf945fb40b4d01dcf371de12cc1fde85acbaa67fa86d3e2cb2c2b70"])
print(result)