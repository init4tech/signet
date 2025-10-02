// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity 0.8.26;

// test contract
import {RollupOrders} from "zenith/src/orders/RollupOrders.sol";
import {IOrders} from "zenith/src/orders/IOrders.sol";
// utils
import {AllocsTest} from "./AllocsTest.t.sol";
import {TestERC20} from "zenith/test/Helpers.t.sol";
import {OrderOriginPermit2Test} from "zenith/test/Permit2Orders.t.sol";

contract AllocsPermit2OrderTest is AllocsTest, OrderOriginPermit2Test {
    function setUp() public override {
        vm.loadAllocs(ALLOCS_FILE_PATH);
        vm.chainId(CHAIN_ID);

        target = RollupOrders(payable(ROLLUP_ORDERS));
        permit2Contract = PERMIT2;

        _mintTokens(owner, amount * 1000);
        _approveTokens(owner, amount * 1000, permit2Contract);

        // setup permit2 contract & permit details
        _setupBatchPermit(token, amount);

        // setup Order Inputs/Outputs
        IOrders.Input memory input = IOrders.Input(token, amount);
        inputs.push(input);

        IOrders.Output memory output = IOrders.Output(token, amount, recipient, chainId);
        outputs.push(output);

        // construct Orders witness
        witness = target.outputWitness(outputs);

        // sign permit + witness
        permit2.signature = signPermit(ownerKey, address(target), permit2.permit, witness);
    }

    // run tests written for local chain env, pointed to contracts deployed in genesis file
}
